//! A domain (and trust) annotation on a `wire` and on a `let` (#211).
//!
//! Source: ADR-0088 and domain-inference.md K1: a `wire`, a module `let`
//! and a `reg` may carry `@D`; "the annotation in a declaration FIXES the
//! signal's domain and is CHECKED: a driver or a value in another domain
//! is E3001"; ADR-0088's table gives `wire x : T @A` ("the wire is in A;
//! its drivers are checked by K6") and `let x : T @A = e` ("e's domain
//! must be A") the same check. K11 (ADR-0052): "a signal's level is its
//! domain's level: a `@D` declaration whose domain has a trust_level
//! takes that level", for both. type-inference.md §3.9 lists the typed
//! `let` and the assignment target as the same check context; and
//! sv-mapping.md §5.2 and §16.3 lower both to a declared net driven by a
//! continuous assignment. So, at module level,
//!
//! ```text
//! wire w : T @D          let w : T @D = e
//! w = e
//! ```
//!
//! are one design. Only that shape is rewritten: a module-level `wire`
//! with a type and an annotation whose next statement is its one driver,
//! a module-level assignment to the whole name, and a module-level `let`
//! with a type and an annotation. The name is assigned nowhere else and
//! not read before the assignment (a `let` is visible only after it,
//! name-resolution.md §5). A wire driven from `comb` is not rewritten:
//! whether `let` and `wire` + `comb` are one design is #291.
//!
//! The location map (ADR-0117 §2, §3): the name, the type with its
//! annotation and the value are copies, anchored to the original's (the
//! name of the `let` to the wire's declaration); only the keywords and the
//! punctuation are the rewrite's own text.

use crate::metamorphic::edit::{Builder, Edit};
use crate::metamorphic::lex::{self, Kind, Tok};

/// For every token, the index of the `{` of its innermost block.
fn parents(src: &str, toks: &[Tok]) -> Vec<Option<usize>> {
    let mut stack = Vec::new();
    let mut out = Vec::with_capacity(toks.len());
    for (k, t) in toks.iter().enumerate() {
        out.push(stack.last().copied());
        match lex::text(src, t) {
            "{" => stack.push(k),
            "}" => {
                stack.pop();
            }
            _ => {}
        }
    }
    out
}

/// Is `open` the `{` of a module body (a `module` token on its line)?
fn is_module_body(src: &str, toks: &[Tok], open: usize) -> bool {
    let mut s = open;
    while s > 0 && !toks[s].line_start {
        s -= 1;
    }
    (s..open).any(|m| lex::text(src, &toks[m]) == "module")
}

/// `name : T @D` from `k` (the name): the end (exclusive) of `@D`.
fn typed_annot(src: &str, toks: &[Tok], k: usize) -> Option<usize> {
    let t = |m: usize| toks.get(m).map(|x| lex::text(src, x));
    if toks.get(k)?.kind != Kind::Ident || lex::is_keyword(t(k)?) || t(k + 1)? != ":" {
        return None;
    }
    let at = (k + 2..toks.len())
        .take_while(|&m| !toks[m].line_start && !matches!(t(m), Some("=" | ";" | "{" | "}")))
        .find(|&m| t(m) == Some("@"))?;
    let d = at + 1;
    (at > k + 2 && toks.get(d)?.kind == Kind::Ident && t(d + 1) != Some("(")).then_some(d + 1)
}

/// The end (exclusive) of the expression that starts at `from`: the next
/// token at bracket depth 0 that starts a line or is `;`, or the `}` that
/// closes the enclosing block.
fn expr_end(src: &str, toks: &[Tok], from: usize) -> usize {
    let mut depth = 0i32;
    for (m, x) in toks.iter().enumerate().skip(from) {
        let s = lex::text(src, x);
        if m > from && depth == 0 && (x.line_start || s == ";") {
            return m;
        }
        match s {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth < 0 {
                    return m;
                }
            }
            _ => {}
        }
    }
    toks.len()
}

/// `wire w : T @D` + `w = e` → `let w : T @D = e`.
pub fn wire_to_let(src: &str) -> Vec<Edit> {
    let toks = lex::lex(src);
    let par = parents(src, &toks);
    let t = |m: usize| lex::text(src, &toks[m]);
    let mut edits = Vec::new();
    for k in 0..toks.len() {
        let site = (|| {
            let open = par[k]?;
            if !(t(k) == "wire" && toks[k].line_start && is_module_body(src, &toks, open)) {
                return None;
            }
            let decl_end = typed_annot(src, &toks, k + 1)?;
            // The next statement, on its own line, is the assignment.
            let a = if toks.get(decl_end).map(|x| lex::text(src, x)) == Some(";") {
                decl_end + 1
            } else {
                decl_end
            };
            if !toks.get(a)?.line_start
                || t(a) != t(k + 1)
                || toks.get(a + 1).map(|x| lex::text(src, x)) != Some("=")
            {
                return None;
            }
            let close = lex::closing(src, &toks, open)?;
            // No other statement starts with the name (a second driver,
            // a driver of a part).
            let other = (open..close).any(|m| {
                m != a
                    && m != k + 1
                    && toks[m].kind == Kind::Ident
                    && t(m) == t(k + 1)
                    && (toks[m].line_start || matches!(t(m - 1), "{" | "}" | ";"))
            });
            let end = expr_end(src, &toks, a + 2);
            (!other && end > a + 2).then_some((decl_end, a, end))
        })();
        let Some((decl_end, a, end)) = site else {
            continue;
        };
        let mut b = Builder::new(src);
        b.lit("let ")
            .copy(toks[k + 1].start, toks[k + 1].end)
            .lit(" : ")
            .copy(toks[k + 3].start, toks[decl_end - 1].end)
            .lit(" = ")
            .copy(toks[a + 2].start, toks[end - 1].end);
        edits.push(b.finish(toks[k].start, toks[end - 1].end));
    }
    edits
}

/// `let w : T @D = e` (module level) → `wire w : T @D` + `w = e`.
pub fn let_to_wire(src: &str) -> Vec<Edit> {
    let toks = lex::lex(src);
    let par = parents(src, &toks);
    let t = |m: usize| lex::text(src, &toks[m]);
    let mut edits = Vec::new();
    for k in 0..toks.len() {
        let site = (|| {
            let open = par[k]?;
            if !(t(k) == "let" && toks[k].line_start && is_module_body(src, &toks, open)) {
                return None;
            }
            let decl_end = typed_annot(src, &toks, k + 1)?;
            if toks.get(decl_end).map(|x| lex::text(src, x)) != Some("=") {
                return None;
            }
            let end = expr_end(src, &toks, decl_end + 1);
            (end > decl_end + 1).then_some((decl_end, end))
        })();
        let Some((decl_end, end)) = site else {
            continue;
        };
        let ind = lex::indent_of(src, toks[k].start);
        let name = (toks[k + 1].start, toks[k + 1].end);
        let mut b = Builder::new(src);
        b.lit("wire ")
            .copy(name.0, name.1)
            .lit(" : ")
            .copy(toks[k + 3].start, toks[decl_end - 1].end)
            .lit(&format!("\n{ind}"))
            .copy(name.0, name.1)
            .lit(" = ")
            .copy(toks[decl_end + 1].start, toks[end - 1].end);
        edits.push(b.finish(toks[k].start, toks[end - 1].end));
    }
    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metamorphic::edit::apply;

    const WIRE: &str = "module M {\n    in  key : u8 @Sec\n    out o : u8 @Sec\n    wire w : u8 @Sec\n    w = key ^ 1\n    o = w\n}\n";
    const LET: &str = "module M {\n    in  key : u8 @Sec\n    out o : u8 @Sec\n    let w : u8 @Sec = key ^ 1\n    o = w\n}\n";

    #[test]
    fn wire_becomes_let() {
        let r = apply(WIRE, wire_to_let(WIRE));
        assert_eq!(r.text, LET);
        // The annotation and the value are anchored to the original's;
        // the keyword maps back to the declaration and its assignment.
        let sec = r.text.find("@Sec =").unwrap();
        let old = WIRE.find("@Sec\n    w =").unwrap();
        assert_eq!(r.to_orig(sec), (old, old));
        let key = r.text.find("key ^").unwrap();
        let old = WIRE.find("key ^").unwrap();
        assert_eq!(r.to_orig(key), (old, old));
        let to = WIRE.find(" ^ 1").unwrap() + 4;
        assert_eq!(
            r.to_orig(r.text.find("let w").unwrap()),
            (WIRE.find("wire").unwrap(), to)
        );
    }

    #[test]
    fn let_becomes_wire() {
        let r = apply(LET, let_to_wire(LET));
        assert_eq!(r.text, WIRE);
    }

    #[test]
    fn not_sites() {
        for src in [
            // No annotation, no type, a block let, an instance.
            "module M {\n    wire w : u8\n    w = a\n    let v : u8 = a\n}\n",
            "module M {\n    let v = a\n    let c = Child { x: a }\n}\n",
            "module M {\n    in clk : clock\n    comb {\n        let v : u8 @A = a\n    }\n}\n",
            // A wire driven from comb (#291), by parts, or twice.
            "module M {\n    wire w : u8 @A\n    comb { w = a }\n}\n",
            "module M {\n    wire w : u8 @A\n    w[0] = a\n}\n",
            "module M {\n    wire w : u8 @A\n    w = a\n    comb { w = b }\n}\n",
            // A wire read before its assignment, a declaration and its
            // driver apart.
            "module M {\n    wire w : u8 @A\n    o = w\n    w = a\n}\n",
            "module M {\n    wire w : u8 @A\n    let v = a\n    w = v\n}\n",
            // An attribute, not a domain.
            "module M {\n    wire w : u8 @max_fanout(4)\n    w = a\n}\n",
        ] {
            assert!(wire_to_let(src).is_empty(), "{src}");
            assert!(let_to_wire(src).is_empty(), "{src}");
        }
    }

    #[test]
    fn multi_line_value() {
        let src = "module M {\n    let w : u8 @A = if c {\n        a\n    } else {\n        b\n    }\n    o = w\n}\n";
        let r = apply(src, let_to_wire(src));
        assert_eq!(
            r.text,
            "module M {\n    wire w : u8 @A\n    w = if c {\n        a\n    } else {\n        b\n    }\n    o = w\n}\n"
        );
        let back = apply(&r.text, wire_to_let(&r.text)).text;
        assert_eq!(back, src);
    }
}

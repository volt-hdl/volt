//! Field-init and port-binding shorthand and the long form (#204).
//!
//! Source: grammar-full.ebnf, `FieldInit` and `PortBind`: "the field `x`
//! (and the binding `x` in PortBind) means exactly `x: x`; the value is the
//! resolution of the name x at that place (Path(x)), and every check
//! applies to both forms alike".

use crate::metamorphic::edit::{Builder, Edit};
use crate::metamorphic::lex::{self, Kind, Tok};

/// Tokens before a path that put it in expression position: the path
/// followed by `{` is then a struct literal or a module instance.
const EXPR_BEFORE: &[&str] = &["=", "<=", "(", ",", ":", "=>", "["];

/// Keywords that open a declaration on their line.
const DECLARES: &[&str] = &[
    "enum", "struct", "bundle", "domain", "module", "fn", "type", "trait", "impl",
];

/// One `Path [<...>] { fields }` group: the tokens of each field, split at
/// the top-level commas.
struct Group {
    fields: Vec<(usize, usize)>,
}

/// Every struct literal and instance body of `src`.
fn groups(src: &str, toks: &[Tok]) -> Vec<Group> {
    let t = |k: usize| lex::text(src, &toks[k]);
    let mut out = Vec::new();
    let mut k = 0;
    while k < toks.len() {
        // The first identifier of a path in expression position.
        let starts_path = toks[k].kind == Kind::Ident
            && !lex::is_keyword(t(k))
            && k > 0
            && EXPR_BEFORE.contains(&t(k - 1));
        // A declaration (`enum G : G {`, `struct S {`) is not a literal.
        let mut ls = k;
        while ls > 0 && !toks[ls].line_start {
            ls -= 1;
        }
        let declares = (ls..k).any(|m| DECLARES.contains(&t(m)));
        if !starts_path || declares {
            k += 1;
            continue;
        }
        let mut j = k + 1;
        while j + 1 < toks.len() && t(j) == "::" && toks[j + 1].kind == Kind::Ident {
            j += 2;
        }
        // Generic arguments `<...>`.
        if j < toks.len() && t(j) == "<" {
            let mut depth = 0;
            let mut m = j;
            while m < toks.len() {
                match t(m) {
                    "<" => depth += 1,
                    ">" => depth -= 1,
                    ">>" => depth -= 2,
                    ";" | "{" | "}" | "=" => break,
                    _ => {}
                }
                m += 1;
                if depth <= 0 {
                    break;
                }
            }
            if depth != 0 {
                k += 1;
                continue;
            }
            j = m;
        }
        if j >= toks.len() || t(j) != "{" {
            k += 1;
            continue;
        }
        let Some(close) = lex::closing(src, toks, j) else {
            k += 1;
            continue;
        };
        let top = lex::top_level(src, toks, j + 1, close);
        let mut fields = Vec::new();
        let mut from = j + 1;
        for &m in top.iter().filter(|&&m| t(m) == ",") {
            fields.push((from, m));
            from = m + 1;
        }
        if from < close {
            fields.push((from, close));
        }
        // Every field is `x` or `x: e`; anything else is not a literal.
        let ok = !fields.is_empty()
            && fields.iter().all(|&(a, b)| {
                toks[a].kind == Kind::Ident
                    && !lex::is_keyword(t(a))
                    && (b == a + 1 || (b > a + 2 && t(a + 1) == ":"))
            });
        if ok {
            out.push(Group { fields });
        }
        k = j + 1;
    }
    out
}

/// `S { x }` → `S { x: x }`: every shorthand field, both copies anchored
/// to the name.
pub fn to_long(src: &str) -> Vec<Edit> {
    let toks = lex::lex(src);
    let mut edits = Vec::new();
    for g in groups(src, &toks) {
        for (a, b) in g.fields {
            if b == a + 1 {
                let (s, e) = (toks[a].start, toks[a].end);
                let mut bld = Builder::new(src);
                bld.copy(s, e).lit(": ").copy(s, e);
                edits.push(bld.finish(s, e));
            }
        }
    }
    edits
}

/// `S { x: x }` → `S { x }`: every field whose value is the same single
/// name. The name of the short form maps back to the whole long field.
pub fn to_short(src: &str) -> Vec<Edit> {
    let toks = lex::lex(src);
    let mut edits = Vec::new();
    for g in groups(src, &toks) {
        for (a, b) in g.fields {
            if b == a + 3
                && toks[a + 2].kind == Kind::Ident
                && lex::text(src, &toks[a]) == lex::text(src, &toks[a + 2])
            {
                let mut bld = Builder::new(src);
                bld.lit(lex::text(src, &toks[a]));
                edits.push(bld.finish(toks[a].start, toks[a + 2].end));
            }
        }
    }
    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metamorphic::edit::apply;

    #[test]
    fn short_to_long() {
        let src = "struct S { a : u8, b : u8 }\nenum E { A, B }\nmodule M {\n    let s = S { a, b: 1 }\n    let c = Child<8> { clk, d: x }\n    on clk { if go { r <= q } }\n}\n";
        let r = apply(src, to_long(src));
        assert_eq!(
            r.text,
            "struct S { a : u8, b : u8 }\nenum E { A, B }\nmodule M {\n    let s = S { a: a, b: 1 }\n    let c = Child<8> { clk: clk, d: x }\n    on clk { if go { r <= q } }\n}\n"
        );
        assert_eq!(r.edits(), 2);
    }

    #[test]
    fn long_to_short() {
        let src = "o = S { a: a, b: b + 1, c: (c) }\nlet i = pkg::Child { x: x }\n";
        let r = apply(src, to_short(src));
        assert_eq!(
            r.text,
            "o = S { a, b: b + 1, c: (c) }\nlet i = pkg::Child { x }\n"
        );
    }

    #[test]
    fn declarations_and_blocks_are_not_sites() {
        for src in [
            "struct S { a : u8 }",
            "enum E { A, B }",
            "enum G : G {\n    Idle\n}",
            "pub enum H : u2 { A, B }",
            "domain D { clock = posedge }",
            "for i in 0..N { x }",
            "if go { x }",
            "match s { a => b }",
            "module M { in a : u8 }",
            "test \"t\" { step(1) }",
        ] {
            assert!(to_long(src).is_empty(), "{src}");
            assert!(to_short(src).is_empty(), "{src}");
        }
    }

    #[test]
    fn nested_literals() {
        let src = "o = Outer { inner: Inner { a }, b }";
        let r = apply(src, to_long(src));
        assert_eq!(r.text, "o = Outer { inner: Inner { a: a }, b: b }");
    }
}

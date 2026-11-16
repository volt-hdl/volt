//! A clock written as `clock` and through a type alias (#234).
//!
//! Source: ADR-0070 §6 (aliases are resolved before SV mapping): "`type W
//! = u8`, `type C = clock`, `type A = [u4; 3]` and chains (`type V = W`)
//! work like the type itself in a port, a reg, a wire and a let"; a
//! non-generic alias is its target (grammar-full.ebnf `TypeAlias`). So
//!
//! ```text
//! in clk : clock          type C = clock;  in clk : C
//! ```
//!
//! are one design. `clock-to-alias` writes every `: clock` of a port,
//! `wire`, `reg` or `let` declaration through one new alias, declared
//! before the first item of the file; `alias-to-clock` writes `clock` for
//! every use of a non-public `type X = clock` whose every use is such a
//! declaration type, and removes the alias. Other positions (a generic
//! argument, a struct field, a `fn` parameter) are not rewritten: ADR-0070
//! names the four declarations.

use crate::metamorphic::edit::{Builder, Edit};
use crate::metamorphic::lex::{self, Kind, Tok};

/// Declarations whose type may be `clock` (ADR-0070 §6).
const DECLS: &[&str] = &["in", "out", "inout", "wire", "reg", "let"];

/// Items that may open a file after its `use` lines.
const ITEMS: &[&str] = &[
    "module", "extern", "pub", "@", "struct", "enum", "domain", "fn", "const", "type", "bundle",
    "test", "trait", "impl",
];

/// Is token `k` the type of a declaration: `: k` on a line that starts
/// with one of `DECLS`?
fn declared_type(src: &str, toks: &[Tok], k: usize) -> bool {
    if k == 0 || lex::text(src, &toks[k - 1]) != ":" {
        return false;
    }
    let mut s = k;
    while s > 0 && !toks[s].line_start {
        s -= 1;
    }
    DECLS.contains(&lex::text(src, &toks[s]))
}

/// The byte where a new item goes: the start of the line of the first
/// item at brace depth 0.
fn first_item(src: &str, toks: &[Tok]) -> Option<usize> {
    let mut depth = 0i32;
    for t in toks {
        let s = lex::text(src, t);
        if depth == 0 && t.line_start && ITEMS.contains(&s) {
            return Some(src[..t.start].rfind('\n').map_or(0, |n| n + 1));
        }
        match s {
            "{" => depth += 1,
            "}" => depth -= 1,
            _ => {}
        }
    }
    None
}

/// `in clk : clock` → `type MetaClock = clock;` + `in clk : MetaClock`.
pub fn clock_to_alias(src: &str) -> Vec<Edit> {
    let toks = lex::lex(src);
    let sites: Vec<usize> = (0..toks.len())
        .filter(|&k| lex::text(src, &toks[k]) == "clock" && declared_type(src, &toks, k))
        .collect();
    let Some(at) = first_item(src, &toks).filter(|_| !sites.is_empty()) else {
        return Vec::new();
    };
    let mut name = String::from("MetaClock");
    while src.contains(&name) {
        name.push('X');
    }
    let mut decl = Builder::new(src);
    decl.lit(&format!("type {name} = clock;\n"));
    let mut edits = vec![decl.finish(at, at)];
    for k in sites {
        let mut b = Builder::new(src);
        b.lit(&name);
        edits.push(b.finish(toks[k].start, toks[k].end));
    }
    edits
}

/// `type C = clock` + `in clk : C` → `in clk : clock`, the alias removed.
pub fn alias_to_clock(src: &str) -> Vec<Edit> {
    let toks = lex::lex(src);
    let t = |m: usize| toks.get(m).map(|x| lex::text(src, x));
    let mut edits = Vec::new();
    let mut depth = 0i32;
    for k in 0..toks.len() {
        match t(k) {
            Some("{") => depth += 1,
            Some("}") => depth -= 1,
            _ => {}
        }
        // `type X = clock [;]`, alone on its line, at depth 0.
        let alias = depth == 0
            && toks[k].line_start
            && t(k) == Some("type")
            && toks.get(k + 1).is_some_and(|x| x.kind == Kind::Ident)
            && t(k + 2) == Some("=")
            && t(k + 3) == Some("clock");
        if !alias {
            continue;
        }
        let after = if t(k + 4) == Some(";") { k + 5 } else { k + 4 };
        if toks.get(after).is_some_and(|x| !x.line_start) {
            continue;
        }
        let name = t(k + 1).unwrap();
        let uses: Vec<usize> = (0..toks.len())
            .filter(|&m| m != k + 1 && toks[m].kind == Kind::Ident && t(m) == Some(name))
            .collect();
        if uses.is_empty() || !uses.iter().all(|&m| declared_type(src, &toks, m)) {
            continue;
        }
        let line = src[..toks[k].start].rfind('\n').map_or(0, |n| n + 1);
        let end = src[toks[k].start..]
            .find('\n')
            .map_or(src.len(), |n| toks[k].start + n + 1);
        edits.push(Builder::new(src).finish(line, end));
        for m in uses {
            let mut b = Builder::new(src);
            b.lit("clock");
            edits.push(b.finish(toks[m].start, toks[m].end));
        }
    }
    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metamorphic::edit::apply;

    #[test]
    fn clock_becomes_alias() {
        let src = "domain D { clock = posedge }\n@strict_timing\nmodule M {\n    in clk : clock @D\n    reg(clk) r : bool = false\n}\n";
        let r = apply(src, clock_to_alias(src));
        assert_eq!(
            r.text,
            "type MetaClock = clock;\ndomain D { clock = posedge }\n@strict_timing\nmodule M {\n    in clk : MetaClock @D\n    reg(clk) r : bool = false\n}\n"
        );
        // The alias maps back to the `clock` it replaces.
        let new = r.text.find(": MetaClock").unwrap() + 2;
        let old = src.find(": clock").unwrap() + 2;
        assert_eq!(r.to_orig(new), (old, old + 5));
    }

    #[test]
    fn use_lines_stay_first_and_the_name_is_fresh() {
        let src = "use lib::MetaClock\nmodule M {\n    in c : clock\n}\n";
        let r = apply(src, clock_to_alias(src));
        assert_eq!(
            r.text,
            "use lib::MetaClock\ntype MetaClockX = clock;\nmodule M {\n    in c : MetaClockX\n}\n"
        );
    }

    #[test]
    fn alias_becomes_clock() {
        let src = "// head\ntype Clk = clock\ntype Byte = u8\nextern module E {\n    in clk : Clk\n}\nmodule M {\n    in clk : Clk\n    in d : Byte\n}\n";
        let r = apply(src, alias_to_clock(src));
        assert_eq!(
            r.text,
            "// head\ntype Byte = u8\nextern module E {\n    in clk : clock\n}\nmodule M {\n    in clk : clock\n    in d : Byte\n}\n"
        );
    }

    #[test]
    fn round_trip() {
        let src = "module M {\n    in clk : clock\n    in d : u8\n}\n";
        let a = apply(src, clock_to_alias(src)).text;
        assert_eq!(apply(&a, alias_to_clock(&a)).text, src);
    }

    #[test]
    fn not_sites() {
        for src in [
            // A domain field, a generic argument, a fn parameter.
            "domain D { clock = posedge }\nmodule M {\n    let f = F<clock> { }\n}\nfn f(c: clock) -> bool { true }\n",
            // A public alias, an alias used elsewhere than a declaration
            // type, an alias of another type, a chain.
            "pub type C = clock\nmodule M {\n    in c : C\n}\n",
            "type C = clock\ntype D = C\nmodule M {\n    in c : D\n}\n",
            "type C = u8\nmodule M {\n    in c : C\n}\n",
        ] {
            assert!(clock_to_alias(src).is_empty(), "{src}");
            assert!(alias_to_clock(src).is_empty(), "{src}");
        }
    }
}

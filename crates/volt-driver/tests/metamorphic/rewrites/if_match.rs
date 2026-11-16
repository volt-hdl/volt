//! A conditional expression on a `bool` and the two-arm match (#215).
//!
//! Source: type-inference.md §3.9 (ADR-0083): "a match expression is a
//! multi-arm `if` expression (§3.7): its width behaviour does not
//! diverge"; its patterns are literals and `_`, the first matching arm
//! wins, and a `_` arm makes it exhaustive (grammar-full.ebnf
//! `MatchExpr`). domain-inference.md K5 (ADR-0083 decision 8): the
//! condition of an `if` and the subject of a match join the result alike.
//! So, for a `bool` condition `c`,
//!
//! ```text
//! if c { a } else { b }
//! match c { true => a, _ => b }
//! ```
//!
//! are one design. Only expression forms that are a whole operand are
//! rewritten: after `=`, `<=`, `(`, `,` or `:`, and followed by the end
//! of the line, `)`, `,`, `;` or `}`. An `else if` chain becomes the `_`
//! arm unchanged. The statement forms (`if c { y = a } else { y = b }`)
//! are not rewritten: §3.9 speaks of the expression.
//!
//! The rewrite cannot see types: a condition that is not a `bool` is an
//! error in both forms (E2001 for `if`, a pattern error for `match`), and
//! the codes may then differ; the PR corpus has no such design.

use crate::metamorphic::edit::{Builder, Edit};
use crate::metamorphic::lex::{self, Tok};
use crate::metamorphic::rewrites::match_form::{body_open, expr_arms};

/// The token before an `if` or `match` that makes it a whole operand.
const BEFORE: &[&str] = &["=", "<=", "(", ",", ":"];
/// The token after its last `}` (or the end of the line).
const AFTER: &[&str] = &[")", ",", ";", "}"];

fn whole_operand(src: &str, toks: &[Tok], first: usize, last: usize) -> bool {
    first > 0
        && BEFORE.contains(&lex::text(src, &toks[first - 1]))
        && toks
            .get(last + 1)
            .is_none_or(|n| n.line_start || AFTER.contains(&lex::text(src, n)))
}

/// `if c { a } else ...` at `k`: (the `{` of the then branch, its `}`,
/// the first and the last token of the else part). The else part is a
/// `{ e }` block (the first and last token are its braces) or an `else
/// if` chain (from its `if` to its final `}`).
fn if_expr(src: &str, toks: &[Tok], k: usize) -> Option<(usize, usize, usize, usize)> {
    let open = body_open(src, toks, k)?;
    let close = lex::closing(src, toks, open)?;
    if close == open + 1 || toks.get(close + 1).map(|x| lex::text(src, x)) != Some("else") {
        return None;
    }
    let e = close + 2;
    match toks.get(e).map(|x| lex::text(src, x)) {
        Some("{") => {
            let ec = lex::closing(src, toks, e)?;
            (ec > e + 1).then_some((open, close, e, ec))
        }
        Some("if") => {
            let (_, _, _, last) = if_expr(src, toks, e)?;
            Some((open, close, e, last))
        }
        _ => None,
    }
}

/// `if c { a } else { b }` → `match c { true => a, _ => b }`.
pub fn if_to_match(src: &str) -> Vec<Edit> {
    let toks = lex::lex(src);
    let mut edits = Vec::new();
    let mut k = 0;
    while k < toks.len() {
        if lex::text(src, &toks[k]) != "if" {
            k += 1;
            continue;
        }
        let Some((open, close, e, last)) = if_expr(src, &toks, k) else {
            k += 1;
            continue;
        };
        if !whole_operand(src, &toks, k, last) {
            k += 1;
            continue;
        }
        let mut b = Builder::new(src);
        b.lit("match ")
            .copy(toks[k + 1].start, toks[open - 1].end)
            .lit(" { true => ")
            .copy(toks[open + 1].start, toks[close - 1].end)
            .lit(", _ => ");
        if lex::text(src, &toks[e]) == "{" {
            b.copy(toks[e + 1].start, toks[last - 1].end);
        } else {
            b.copy(toks[e].start, toks[last].end);
        }
        b.lit(" }");
        edits.push(b.finish(toks[k].start, toks[last].end));
        k = last + 1;
    }
    edits
}

/// `match c { true => a, _ => b }` (or `false => b`, in either order) →
/// `if c { a } else { b }`.
pub fn match_to_if(src: &str) -> Vec<Edit> {
    let toks = lex::lex(src);
    let mut edits = Vec::new();
    let mut k = 0;
    while k < toks.len() {
        let site = (|| {
            if lex::text(src, &toks[k]) != "match" {
                return None;
            }
            let open = body_open(src, &toks, k)?;
            let close = lex::closing(src, &toks, open)?;
            if !whole_operand(src, &toks, k, close) {
                return None;
            }
            let arms = expr_arms(src, &toks, open, close)?;
            let [(p0, v0), (p1, v1)] = arms[..] else {
                return None;
            };
            // A pattern of one token: no guard, no alternative.
            let pat = |p: (usize, usize)| &src[p.0..p.1];
            let (then, other) = match (pat(p0), pat(p1)) {
                ("true", "_" | "false") => (v0, v1),
                ("false", "true" | "_") => (v1, v0),
                _ => return None,
            };
            Some((open, close, then, other))
        })();
        let Some((open, close, then, other)) = site else {
            k += 1;
            continue;
        };
        let mut b = Builder::new(src);
        b.lit("if ")
            .copy(toks[k + 1].start, toks[open - 1].end)
            .lit(" { ")
            .copy(then.0, then.1)
            .lit(" } else { ")
            .copy(other.0, other.1)
            .lit(" }");
        edits.push(b.finish(toks[k].start, toks[close].end));
        k = close + 1;
    }
    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metamorphic::edit::apply;

    #[test]
    fn if_becomes_match() {
        let src = "module M {\n    y = if sel { a } else { b + 1 }\n    let z : u9 = (if c { a } else if d { b } else { 0 })\n}\n";
        let r = apply(src, if_to_match(src));
        assert_eq!(
            r.text,
            "module M {\n    y = match sel { true => a, _ => b + 1 }\n    let z : u9 = (match c { true => a, _ => if d { b } else { 0 } })\n}\n"
        );
        assert_eq!(r.edits(), 2);
    }

    #[test]
    fn match_becomes_if() {
        let src = "comb {\n    y = match fs { true => sa, _ => 0 }\n    z = match k { false => 1, true => 2 }\n    w = match k {\n        true => p\n        false => q\n    }\n}\n";
        let r = apply(src, match_to_if(src));
        assert_eq!(
            r.text,
            "comb {\n    y = if fs { sa } else { 0 }\n    z = if k { 2 } else { 1 }\n    w = if k { p } else { q }\n}\n"
        );
    }

    #[test]
    fn round_trip() {
        let src = "y = if sel { a } else { b }\n";
        let m = apply(src, if_to_match(src)).text;
        assert_eq!(apply(&m, match_to_if(&m)).text, src);
    }

    #[test]
    fn not_sites() {
        for src in [
            // Statements.
            "comb {\n    if c { y = a } else { y = b }\n}\n",
            "on clk {\n    if go { r <= d }\n}\n",
            // An operand of an operator.
            "y = 1 + if c { a } else { b }\n",
            "y = if c { a } else { b } + 1\n",
            // No else (an error in both forms, E0001).
            "y = if c { a }\n",
            // A statement match, a match on a number, a guard, three arms.
            "comb {\n    match c {\n        true => { y = a }\n        _ => { y = b }\n    }\n}\n",
            "y = match s { 0 => a, _ => b }\n",
            "y = match c { true if d => a, _ => b }\n",
            "y = match c { true => a, false => b, _ => 0 }\n",
        ] {
            assert!(if_to_match(src).is_empty(), "{src}");
            assert!(match_to_if(src).is_empty(), "{src}");
        }
    }

    #[test]
    fn map_back_to_the_branches() {
        let src = "y = if c { a } else { bb }\n";
        let r = apply(src, if_to_match(src));
        let new = r.text.find("bb").unwrap();
        let old = src.find("bb").unwrap();
        assert_eq!(r.to_orig(new), (old, old));
        // `true =>` is the rewrite's own text: the whole `if` maps back.
        let own = r.text.find("true").unwrap();
        assert_eq!(r.to_orig(own), (4, src.len() - 1));
    }
}

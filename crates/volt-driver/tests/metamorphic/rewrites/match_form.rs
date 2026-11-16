//! Statement match and expression match (#216).
//!
//! Source: sv-mapping.md §19.1 (ADR-0083): "a match expression that is the
//! whole right-hand side of an assignment (root position) is lowered to the
//! statement form; the target is repeated in every arm", root positions
//! including "an assignment inside `on`/`comb` (`<=`/`=`)"; and
//! grammar-full.ebnf, `MatchExpr`: "the expression match follows the SAME
//! rules as the statement (ADR-0083)". So, inside an `on` or `comb` block,
//!
//! ```text
//! t op match s { p1 => e1, p2 => e2 }
//! match s { p1 => { t op e1 } p2 => { t op e2 } }
//! ```
//!
//! are one design. Only whole statements are rewritten: a target that is
//! a name with field or index suffixes, every arm one assignment to that
//! same target with the same operator, on one line.

use crate::metamorphic::edit::{Builder, Edit};
use crate::metamorphic::lex::{self, Kind, Tok};

/// For every token: is it inside an `on` or `comb` block?
fn in_process(src: &str, toks: &[Tok]) -> Vec<bool> {
    let mut stack: Vec<bool> = Vec::new();
    let mut out = Vec::with_capacity(toks.len());
    for (k, t) in toks.iter().enumerate() {
        out.push(stack.last().copied().unwrap_or(false));
        match lex::text(src, t) {
            "{" => {
                // The first token of the line that opens the block.
                let mut s = k;
                while s > 0 && !toks[s].line_start {
                    s -= 1;
                }
                let label = lex::text(src, &toks[s]);
                let parent = stack.last().copied().unwrap_or(false);
                stack.push(parent || label == "on" || label == "comb");
            }
            "}" => {
                stack.pop();
            }
            _ => {}
        }
    }
    out
}

/// A statement starts at `k`: first on its line, or after `{`, `}`, `;`.
fn starts_statement(src: &str, toks: &[Tok], k: usize) -> bool {
    toks[k].line_start || (k > 0 && matches!(lex::text(src, &toks[k - 1]), "{" | "}" | ";"))
}

/// The end (exclusive) of an assignment target starting at `k`:
/// `name { .field | [..] }`.
fn lvalue_end(src: &str, toks: &[Tok], k: usize) -> Option<usize> {
    if toks[k].kind != Kind::Ident || lex::is_keyword(lex::text(src, &toks[k])) {
        return None;
    }
    let mut j = k + 1;
    loop {
        match toks.get(j).map(|t| lex::text(src, t)) {
            Some(".") if toks.get(j + 1).is_some_and(|t| t.kind == Kind::Ident) => j += 2,
            Some("[") => j = lex::closing(src, toks, j)? + 1,
            _ => return Some(j),
        }
    }
}

/// `match <subject> {`: the index of the `{` that opens the body.
fn body_open(src: &str, toks: &[Tok], match_tok: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (k, t) in toks.iter().enumerate().skip(match_tok + 1) {
        match lex::text(src, t) {
            "(" | "[" => depth += 1,
            ")" | "]" => depth -= 1,
            "{" if depth == 0 => return (k > match_tok + 1).then_some(k),
            "}" | ";" => return None,
            _ => {}
        }
    }
    None
}

/// The byte ranges of a statement arm: (pattern, target, op, value).
type StmtArm = ((usize, usize), (usize, usize), String, (usize, usize));

fn span(toks: &[Tok], from: usize, to: usize) -> (usize, usize) {
    (toks[from].start, toks[to - 1].end)
}

/// `match s { p => { t op e } ... }` → `t op match s { p => e, ... }`.
pub fn stmt_to_expr(src: &str) -> Vec<Edit> {
    let toks = lex::lex(src);
    let proc = in_process(src, &toks);
    let t = |k: usize| lex::text(src, &toks[k]);
    let mut edits = Vec::new();
    let mut k = 0;
    while k < toks.len() {
        if !(t(k) == "match" && proc[k] && starts_statement(src, &toks, k)) {
            k += 1;
            continue;
        }
        let Some(open) = body_open(src, &toks, k) else {
            k += 1;
            continue;
        };
        let Some(close) = lex::closing(src, &toks, open) else {
            k += 1;
            continue;
        };
        let mut arms: Vec<StmtArm> = Vec::new();
        let mut a = open + 1;
        let mut ok = true;
        while a < close {
            let Some(arrow) = (a..close).find(|&m| t(m) == "=>") else {
                ok = false;
                break;
            };
            let bo = arrow + 1;
            if arrow == a || bo >= close || t(bo) != "{" {
                ok = false;
                break;
            }
            let Some(bc) = lex::closing(src, &toks, bo) else {
                ok = false;
                break;
            };
            // One statement on one line: `t op e [;]`.
            let mut end = bc;
            if end > bo + 1 && t(end - 1) == ";" {
                end -= 1;
            }
            let lv_end = (bo + 1 < end)
                .then(|| lvalue_end(src, &toks, bo + 1))
                .flatten();
            let one_line = (bo..=bc).all(|m| toks[m].line == toks[bo].line);
            let Some(lv_end) = lv_end.filter(|&e| e + 1 < end && one_line) else {
                ok = false;
                break;
            };
            let op = t(lv_end);
            let rest_has_stmt = (lv_end + 1..end).any(|m| matches!(t(m), ";" | "=" | "{" | "}"));
            if !matches!(op, "=" | "<=") || rest_has_stmt {
                ok = false;
                break;
            }
            arms.push((
                span(&toks, a, arrow),
                span(&toks, bo + 1, lv_end),
                op.to_string(),
                span(&toks, lv_end + 1, end),
            ));
            a = bc + 1;
            if a < close && t(a) == "," {
                a += 1;
            }
        }
        let same = |i: usize| {
            src[arms[i].1 .0..arms[i].1 .1] == src[arms[0].1 .0..arms[0].1 .1]
                && arms[i].2 == arms[0].2
        };
        if !ok || arms.is_empty() || !(0..arms.len()).all(same) {
            k = close + 1;
            continue;
        }
        let ind = lex::indent_of(src, toks[k].start);
        let mut b = Builder::new(src);
        b.copy(arms[0].1 .0, arms[0].1 .1)
            .lit(&format!(" {} match ", arms[0].2))
            .copy(toks[k + 1].start, toks[open - 1].end)
            .lit(" {\n");
        for (pat, _, _, val) in &arms {
            b.lit(&format!("{ind}    "))
                .copy(pat.0, pat.1)
                .lit(" => ")
                .copy(val.0, val.1)
                .lit(",\n");
        }
        b.lit(&format!("{ind}}}"));
        edits.push(b.finish(toks[k].start, toks[close].end));
        k = close + 1;
    }
    edits
}

/// `t op match s { p => e, ... }` → `match s { p => { t op e } ... }`.
pub fn expr_to_stmt(src: &str) -> Vec<Edit> {
    let toks = lex::lex(src);
    let proc = in_process(src, &toks);
    let t = |k: usize| lex::text(src, &toks[k]);
    let mut edits = Vec::new();
    let mut k = 0;
    while k < toks.len() {
        let site = (|| {
            if !(proc[k] && starts_statement(src, &toks, k)) {
                return None;
            }
            let lv_end = lvalue_end(src, &toks, k)?;
            let op = t(lv_end);
            if !matches!(op, "=" | "<=")
                || toks.get(lv_end + 1).map(|x| lex::text(src, x)) != Some("match")
            {
                return None;
            }
            let m = lv_end + 1;
            let open = body_open(src, &toks, m)?;
            let close = lex::closing(src, &toks, open)?;
            // The match is the whole statement.
            if let Some(next) = toks.get(close + 1) {
                if !(next.line_start || matches!(lex::text(src, next), ";" | "}")) {
                    return None;
                }
            }
            let top = lex::top_level(src, &toks, open + 1, close);
            let arrows: Vec<usize> = top.iter().copied().filter(|&x| t(x) == "=>").collect();
            if arrows.is_empty() {
                return None;
            }
            let mut arms = Vec::new();
            let mut a = open + 1;
            for (i, &arrow) in arrows.iter().enumerate() {
                if arrow <= a {
                    return None;
                }
                let limit = arrows.get(i + 1).copied().unwrap_or(close);
                let comma = top
                    .iter()
                    .copied()
                    .find(|&x| x > arrow && x < limit && t(x) == ",");
                let (end, next) = match (comma, arrows.get(i + 1)) {
                    (Some(c), _) => (c, c + 1),
                    (None, None) => (close, close),
                    (None, Some(&next_arrow)) => {
                        // No comma: the next pattern starts its own line.
                        let mut s = next_arrow;
                        while s > arrow && !toks[s].line_start {
                            s -= 1;
                        }
                        if s <= arrow + 1 {
                            return None;
                        }
                        (s, s)
                    }
                };
                if end <= arrow + 1 || t(arrow + 1) == "{" {
                    return None;
                }
                arms.push((span(&toks, a, arrow), span(&toks, arrow + 1, end)));
                a = next;
            }
            if a != close {
                return None;
            }
            Some((lv_end, m, open, close, arms))
        })();
        let Some((lv_end, m, open, close, arms)) = site else {
            k += 1;
            continue;
        };
        let ind = lex::indent_of(src, toks[k].start);
        let op = t(lv_end);
        let mut b = Builder::new(src);
        b.lit("match ")
            .copy(toks[m + 1].start, toks[open - 1].end)
            .lit(" {\n");
        for (pat, val) in &arms {
            b.lit(&format!("{ind}    "))
                .copy(pat.0, pat.1)
                .lit(" => { ")
                .copy(toks[k].start, toks[lv_end - 1].end)
                .lit(&format!(" {op} "))
                .copy(val.0, val.1)
                .lit(" }\n");
        }
        b.lit(&format!("{ind}}}"));
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
    fn statement_to_expression() {
        let src = "module M {\n    comb {\n        match s {\n            0 => { y = 1 }\n            _ => { y = a + 1; }\n        }\n    }\n}\n";
        let r = apply(src, stmt_to_expr(src));
        assert_eq!(
            r.text,
            "module M {\n    comb {\n        y = match s {\n            0 => 1,\n            _ => a + 1,\n        }\n    }\n}\n"
        );
    }

    #[test]
    fn expression_to_statement() {
        let src = "module M {\n    on clk {\n        q <= match mode { Mode::Idle => q, Mode::Load => d, _ => q << 1 }\n    }\n}\n";
        let r = apply(src, expr_to_stmt(src));
        assert_eq!(
            r.text,
            "module M {\n    on clk {\n        match mode {\n            Mode::Idle => { q <= q }\n            Mode::Load => { q <= d }\n            _ => { q <= q << 1 }\n        }\n    }\n}\n"
        );
    }

    #[test]
    fn arms_without_commas_end_at_the_next_line() {
        let src = "comb {\n    y = match s {\n        0 => a\n        1 | 2 => f(b, c)\n        _ => 0\n    }\n}\n";
        let r = apply(src, expr_to_stmt(src));
        assert_eq!(
            r.text,
            "comb {\n    match s {\n        0 => { y = a }\n        1 | 2 => { y = f(b, c) }\n        _ => { y = 0 }\n    }\n}\n"
        );
    }

    #[test]
    fn round_trip_keeps_the_arms() {
        let src = "comb {\n    match s {\n        0 => { y = 1 }\n        _ => { y = 7 }\n        1 => { y = 9 }\n    }\n}\n";
        let e = apply(src, stmt_to_expr(src)).text;
        let back = apply(&e, expr_to_stmt(&e)).text;
        assert_eq!(back, src);
    }

    #[test]
    fn not_sites() {
        for src in [
            // Module level: the statement form would need a comb block.
            "module M {\n    y = match s { 0 => 1, _ => 2 }\n}\n",
            // A let: the statement form needs a declared signal.
            "comb {\n    let t = match s { 0 => 1, _ => 2 }\n}\n",
            // Inner position: an operand.
            "comb {\n    y = 1 + match s { 0 => 1, _ => 2 }\n}\n",
            // Arms that assign different targets, or more than one statement.
            "comb {\n    match s {\n        0 => { y = 1 }\n        _ => { z = 1 }\n    }\n}\n",
            "on clk {\n    match s {\n        0 => { a <= 1; b <= 2 }\n        _ => { a <= 0 }\n    }\n}\n",
            "on clk {\n    match s {\n        0 => {}\n        _ => { a <= 0 }\n    }\n}\n",
        ] {
            assert!(stmt_to_expr(src).is_empty(), "{src}");
            assert!(expr_to_stmt(src).is_empty(), "{src}");
        }
    }

    #[test]
    fn map_back_to_the_arm_values() {
        let src =
            "comb {\n    match s {\n        0 => { y = a }\n        _ => { y = b }\n    }\n}\n";
        let r = apply(src, stmt_to_expr(src));
        let b_new = r.text.rfind('b').unwrap();
        let b_old = src.rfind('b').unwrap();
        assert_eq!(r.to_orig(b_new), (b_old, b_old));
    }
}

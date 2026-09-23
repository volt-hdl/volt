//! Açık sınırlı sayaç tanıma ve kontratları (ADR-0066 C2/C3).
//!
//! Sayaç: `uN` register'ı; her yazması ya sabit (`r <= 0`) ya da
//! `r <= r + 1`, ve HER artış bir sınır karşılaştırmasının korumasında:
//!
//! * `else` kolunda: `if r == E` / `if r >= E` (ya da bunları içeren `||`),
//! * `then` kolunda: `if r != E` / `if r < E` (ya da bunları içeren `&&`).
//!
//! `E` sabit ifadedir (literal, `const`, aritmetik) ve bütün artışlarda
//! aynı değerdedir. Reset değeri ve sabit yazmalar `E`'yi aşmaz. O zaman
//! `r <= E` tümevarımsal olarak tutar (artış yalnız `r < E` iken olur).
//!
//! * C2 `invariant: r <= E` — `E` tipin en büyük değerinden küçükse
//!   (yoksa tanım gereği doğrudur, üretilmez).
//! * C3 `cover: r == E` — sarma noktasına ulaşılır.
//! * C1 (genişlik sınırı) ve C4 (etkinsizken değişmez) üretilmez:
//!   C1 `uN` sarması yüzünden anlamsız, C4 kodun yeniden yazımıdır.

use std::collections::HashMap;

use volt_ast::{AutoRule, BinOp, ContractKind, Expr, ExprKind, Idx, SourceFile};

use super::super::mono::unroll::eval_const;
use super::gen::{Spec, G};
use super::scan::{Cond, RegInfo, Scan};

const DEPTH0: u32 = 0;

pub(super) fn specs(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    scan: &Scan,
    reg: &RegInfo,
) -> Vec<Spec> {
    let Some(c) = recognize(ast, consts, scan, reg) else {
        return Vec::new();
    };
    let name = &reg.name;
    let subject = format!("wrap check on {name}");
    let mut out = Vec::new();
    if c.bound < super::max_value(reg.width) {
        out.push(Spec {
            kind: ContractKind::Invariant,
            rule: AutoRule::CounterBound,
            expr: G::bin(BinOp::Le, G::Name(name.clone()), G::Copy(c.bound_expr)),
            subject: subject.clone(),
            from: c.from,
        });
    }
    out.push(Spec {
        kind: ContractKind::Cover,
        rule: AutoRule::CounterWrap,
        expr: G::bin(BinOp::Eq, G::Name(name.clone()), G::Copy(c.bound_expr)),
        subject,
        from: c.from,
    });
    out
}

struct Counter {
    bound: i128,
    bound_expr: Idx<Expr>,
    /// İlk artışı koruyan karşılaştırma.
    from: volt_span::Span,
}

fn recognize(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    scan: &Scan,
    reg: &RegInfo,
) -> Option<Counter> {
    let name = reg.name.as_str();
    let writes = scan.writes.get(name)?;
    let max = super::max_value(reg.width);
    let mut consts_written = vec![eval_const(ast, consts, reg.init, DEPTH0)?];
    let mut found: Option<Counter> = None;
    for w in writes {
        if is_increment(ast, name, w.rhs) {
            let (e, cond) = w
                .conds
                .iter()
                .rev()
                .find_map(|&c| bound_of(ast, consts, &scan.locals, name, c))?;
            let v = eval_const(ast, consts, e, DEPTH0)?;
            match &found {
                None => {
                    found = Some(Counter {
                        bound: v,
                        bound_expr: e,
                        from: ast.exprs[cond].span,
                    })
                }
                Some(c) if c.bound == v => {}
                Some(_) => return None,
            }
        } else if super::is_const_expr(ast, &scan.locals, w.rhs) {
            consts_written.push(eval_const(ast, consts, w.rhs, DEPTH0)?);
        } else {
            return None;
        }
    }
    let c = found?;
    let ok =
        (0..=max).contains(&c.bound) && consts_written.iter().all(|&k| (0..=c.bound).contains(&k));
    ok.then_some(c)
}

/// `r + 1` ya da `1 + r`.
fn is_increment(ast: &SourceFile, name: &str, e: Idx<Expr>) -> bool {
    let ExprKind::Binary {
        op: BinOp::Add,
        lhs,
        rhs,
    } = ast.exprs[e].kind
    else {
        return false;
    };
    (is_name(ast, lhs, name) && is_one(ast, rhs)) || (is_one(ast, lhs) && is_name(ast, rhs, name))
}

fn is_name(ast: &SourceFile, e: Idx<Expr>, name: &str) -> bool {
    matches!(&ast.exprs[e].kind, ExprKind::Path(p)
        if p.segments.len() == 1 && p.segments[0].text == name)
}

fn is_one(ast: &SourceFile, e: Idx<Expr>) -> bool {
    matches!(ast.exprs[e].kind, ExprKind::IntLit { value: 1, .. })
}

/// Koşul `r`'nin `E`'den küçük olduğunu garanti ediyorsa (E, karşılaştırma).
fn bound_of(
    ast: &SourceFile,
    consts: &HashMap<String, Idx<Expr>>,
    locals: &std::collections::HashSet<String>,
    name: &str,
    cond: Cond,
) -> Option<(Idx<Expr>, Idx<Expr>)> {
    // then: `&&` bağlaçlarından biri r < E / r != E (E > r / E != r);
    // else: `||` ayraçlarından biri r == E / r >= E (E == r / E <= r).
    let (joiner, e, direct, mirrored): (BinOp, _, &[BinOp], &[BinOp]) = match cond {
        Cond::Then(e) => (
            BinOp::And,
            e,
            &[BinOp::Lt, BinOp::Ne],
            &[BinOp::Gt, BinOp::Ne],
        ),
        Cond::Else(e) => (
            BinOp::Or,
            e,
            &[BinOp::Eq, BinOp::Ge],
            &[BinOp::Eq, BinOp::Le],
        ),
    };
    let mut parts = Vec::new();
    split(ast, joiner, e, &mut parts);
    parts.into_iter().find_map(|p| {
        let ExprKind::Binary { op, lhs, rhs } = ast.exprs[p].kind else {
            return None;
        };
        let bound = if direct.contains(&op) && is_name(ast, lhs, name) {
            rhs
        } else if mirrored.contains(&op) && is_name(ast, rhs, name) {
            lhs
        } else {
            return None;
        };
        let constant = super::is_const_expr(ast, locals, bound)
            && eval_const(ast, consts, bound, DEPTH0).is_some();
        constant.then_some((bound, p))
    })
}

/// `a && b && c` → [a, b, c] (joiner = And).
fn split(ast: &SourceFile, joiner: BinOp, e: Idx<Expr>, out: &mut Vec<Idx<Expr>>) {
    match ast.exprs[e].kind {
        ExprKind::Binary { op, lhs, rhs } if op == joiner => {
            split(ast, joiner, lhs, out);
            split(ast, joiner, rhs, out);
        }
        _ => out.push(e),
    }
}

//! Sayısal `match`'te erişilemez kol kuralı (ADR-0075).
//!
//! Muhafızsız bir kolun bütün literal değerleri önceki muhafızsız kollarda
//! geçtiyse kol hiç seçilmez: volt-hir W2014 verir, sv-emit kolu `case`'e
//! yazmaz (enum'daki yinelenen varyantla aynı kural, ADR-0074 Karar 4).
//! Kural tek yerde: iki katman aynı kolu erişilemez sayar. Joker ya da
//! bağlama deseninden sonraki kollar ve literal olmayan alternatifli kollar
//! (yol, tuple) değerlendirilmez.

use crate::{Expr, ExprKind, Idx, MatchStmt, Pattern, PatternKind, SourceFile, UnOp};

/// Desen literalinin değeri: `(negatif, büyüklük)` ya da bool. Yazım
/// biçimi (`1`, `0x1`) değeri değiştirmez; `-0` sıfırdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitKey {
    Int(bool, u128),
    Bool(bool),
}

/// Kol başına "erişilemez" bayrağı (sayısal ya da bool sınanan için).
pub fn unreachable_value_arms(ast: &SourceFile, m: &MatchStmt) -> Vec<bool> {
    let mut seen: Vec<LitKey> = Vec::new();
    let mut wildcard = false;
    let mut out = vec![false; m.arms.len()];
    for (i, arm) in m.arms.iter().enumerate() {
        if arm.guard.is_some() || wildcard {
            continue;
        }
        let mut keys = Vec::new();
        if !literal_keys(ast, arm.pattern, &mut keys) {
            wildcard |= catches_all(ast, arm.pattern);
            continue;
        }
        out[i] = !keys.is_empty() && keys.iter().all(|k| seen.contains(k));
        for k in keys {
            if !seen.contains(&k) {
                seen.push(k);
            }
        }
    }
    out
}

/// Desenin literal değerleri; desen yalnız literallerden (ve onların `|`
/// alternatiflerinden) oluşmuyorsa `false`.
fn literal_keys(ast: &SourceFile, pat: Idx<Pattern>, out: &mut Vec<LitKey>) -> bool {
    match &ast.patterns[pat].kind {
        PatternKind::Or(alts) => alts.iter().all(|&a| literal_keys(ast, a, out)),
        PatternKind::Literal(expr) => match literal_key(ast, *expr) {
            Some(k) => {
                out.push(k);
                true
            }
            None => false,
        },
        _ => false,
    }
}

fn literal_key(ast: &SourceFile, expr: Idx<Expr>) -> Option<LitKey> {
    match &ast.exprs[expr].kind {
        ExprKind::IntLit { value, .. } => Some(LitKey::Int(false, *value)),
        ExprKind::BoolLit(b) => Some(LitKey::Bool(*b)),
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => match literal_key(ast, *operand)? {
            LitKey::Int(_, 0) => Some(LitKey::Int(false, 0)),
            LitKey::Int(neg, v) => Some(LitKey::Int(!neg, v)),
            LitKey::Bool(_) => None,
        },
        _ => None,
    }
}

/// Joker ya da bağlama (ya da onları içeren `|`) her değeri yakalar.
fn catches_all(ast: &SourceFile, pat: Idx<Pattern>) -> bool {
    match &ast.patterns[pat].kind {
        PatternKind::Wildcard | PatternKind::Binding(_) => true,
        PatternKind::Or(alts) => alts.iter().any(|&a| catches_all(ast, a)),
        _ => false,
    }
}

//! Sayısal `match`'te erişilemez kol kuralı (ADR-0075).
//!
//! Muhafızsız bir kolun bütün literal değerleri önceki muhafızsız kollarda
//! geçtiyse kol hiç seçilmez: volt-hir W2014 verir, sv-emit kolu `case`'e
//! yazmaz (enum'daki yinelenen varyantla aynı kural, ADR-0074 Karar 4).
//! Kural tek yerde: iki katman aynı kolu erişilemez sayar. Sabit adı
//! (`LIMIT`, ADR-0085) değerini katmanın kendi sabit değerlendiricisi
//! (`const_value`) verir; değerlendirilemeyen ad kolu değerlendirme dışı
//! bırakır. Joker deseninden sonraki kollar ve literal olmayan
//! alternatifli kollar (yol, tuple) değerlendirilmez.

use crate::{Expr, ExprKind, Idx, MatchArm, Pattern, PatternKind, SourceFile, UnOp};

/// Desen literalinin değeri: `(negatif, büyüklük)` ya da bool. Yazım
/// biçimi (`1`, `0x1`) değeri değiştirmez; `-0` sıfırdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitKey {
    Int(bool, u128),
    Bool(bool),
}

/// Kol başına "erişilemez" bayrağı (sayısal ya da bool sınanan için);
/// deyim ve ifade `match`'i aynı kural (ADR-0083 Karar 1).
///
/// `const_value`: tek parçalı ad deseninin (`LIMIT`) sabit değeri; `None`
/// değerlendirilemez demektir.
pub fn unreachable_value_arms(
    ast: &SourceFile,
    arms: &[MatchArm],
    const_value: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Vec<bool> {
    let mut seen: Vec<LitKey> = Vec::new();
    let mut wildcard = false;
    let mut out = vec![false; arms.len()];
    for (i, arm) in arms.iter().enumerate() {
        if arm.guard.is_some() || wildcard {
            continue;
        }
        let mut keys = Vec::new();
        if !literal_keys(ast, arm.pattern, const_value, &mut keys) {
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
fn literal_keys(
    ast: &SourceFile,
    pat: Idx<Pattern>,
    const_value: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
    out: &mut Vec<LitKey>,
) -> bool {
    match &ast.patterns[pat].kind {
        PatternKind::Or(alts) => alts.iter().all(|&a| literal_keys(ast, a, const_value, out)),
        PatternKind::Literal(expr) => match literal_key(ast, *expr, const_value) {
            Some(k) => {
                out.push(k);
                true
            }
            None => false,
        },
        _ => false,
    }
}

fn literal_key(
    ast: &SourceFile,
    expr: Idx<Expr>,
    const_value: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Option<LitKey> {
    match &ast.exprs[expr].kind {
        ExprKind::IntLit { value, .. } => Some(LitKey::Int(false, *value)),
        ExprKind::BoolLit(b) => Some(LitKey::Bool(*b)),
        ExprKind::Path(p) if p.segments.len() == 1 => {
            let v = const_value(expr)?;
            Some(LitKey::Int(v < 0, v.unsigned_abs()))
        }
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => match literal_key(ast, *operand, const_value)? {
            LitKey::Int(_, 0) => Some(LitKey::Int(false, 0)),
            LitKey::Int(neg, v) => Some(LitKey::Int(!neg, v)),
            LitKey::Bool(_) => None,
        },
        _ => None,
    }
}

/// Joker (ya da onu içeren `|`) her değeri yakalar.
fn catches_all(ast: &SourceFile, pat: Idx<Pattern>) -> bool {
    match &ast.patterns[pat].kind {
        PatternKind::Wildcard => true,
        PatternKind::Or(alts) => alts.iter().any(|&a| catches_all(ast, a)),
        _ => false,
    }
}

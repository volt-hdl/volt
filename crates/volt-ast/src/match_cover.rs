//! `match`'te erişilemez kol kuralı (ADR-0075, sv-mapping.md §19.1).
//!
//! İki neden: (1) ilk muhafızsız joker koldan (`_` ya da onu içeren `|`)
//! sonraki her kol hiç seçilmez — ilk eşleşen kol kazanır, joker her değeri
//! yakalar (#216); (2) muhafızsız bir kolun bütün literal değerleri önceki
//! muhafızsız kollarda geçtiyse kol hiç seçilmez (enum'daki yinelenen
//! varyantla aynı kural, ADR-0074 Karar 4). volt-hir W2014 verir, sv-emit
//! kolu yazmaz — deyim `case`'inde de ifadenin `case`/üçlü zincirinde de.
//! Kural tek yerde: iki katman ve iki biçim aynı kolu erişilemez sayar.
//! Sabit adı (`LIMIT`, ADR-0085) değerini katmanın kendi sabit
//! değerlendiricisi (`const_value`) verir; değerlendirilemeyen ad kolu (2)
//! dışında bırakır. Literal olmayan alternatifli kollar (yol, tuple) (2)'de
//! değerlendirilmez; enum'un varyant kuralı katmanların kendisindedir,
//! jokerden sonrası (1) buradan gelir.

use crate::{Expr, ExprKind, Idx, MatchArm, Pattern, PatternKind, SourceFile, UnOp};

/// Desen literalinin değeri: `(negatif, büyüklük)` ya da bool. Yazım
/// biçimi (`1`, `0x1`) değeri değiştirmez; `-0` sıfırdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitKey {
    Int(bool, u128),
    Bool(bool),
}

/// Kolun neden hiç seçilmediği.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unreachable {
    /// Önceki muhafızsız bir joker kol her değeri yakaladı.
    AfterCatchAll,
    /// Bütün değerleri önceki muhafızsız kollarda geçti.
    Covered,
}

/// Kol başına "ilk muhafızsız joker koldan sonra mı": sınananın tipinden
/// bağımsız (sayı, bool, enum). Muhafızlı kol da sayılır — jokerden sonra
/// hiçbir kol seçilmez; muhafızlı joker her değeri yakalamaz.
pub fn arms_after_catch_all(ast: &SourceFile, arms: &[MatchArm]) -> Vec<bool> {
    let first = arms
        .iter()
        .position(|arm| arm.guard.is_none() && catches_all(ast, arm.pattern));
    (0..arms.len())
        .map(|i| first.is_some_and(|f| i > f))
        .collect()
}

/// Kol başına erişilemezlik nedeni (sayısal ya da bool sınanan için);
/// deyim ve ifade `match`'i aynı kural (ADR-0083 Karar 1).
///
/// `const_value`: tek parçalı ad deseninin (`LIMIT`) sabit değeri; `None`
/// değerlendirilemez demektir.
pub fn unreachable_value_arms(
    ast: &SourceFile,
    arms: &[MatchArm],
    const_value: &mut dyn FnMut(Idx<Expr>) -> Option<i128>,
) -> Vec<Option<Unreachable>> {
    let after = arms_after_catch_all(ast, arms);
    let mut seen: Vec<LitKey> = Vec::new();
    let mut out = vec![None; arms.len()];
    for (i, arm) in arms.iter().enumerate() {
        if after[i] {
            out[i] = Some(Unreachable::AfterCatchAll);
            continue;
        }
        if arm.guard.is_some() {
            continue;
        }
        let mut keys = Vec::new();
        if !literal_keys(ast, arm.pattern, const_value, &mut keys) {
            continue;
        }
        if !keys.is_empty() && keys.iter().all(|k| seen.contains(k)) {
            out[i] = Some(Unreachable::Covered);
        }
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

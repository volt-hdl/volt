//! Tip ve düzen sabitlerinin değerlendirmesi — tek yer.
//!
//! Genişlik (`uint<W>`, `bits<W>`), dizi uzunluğu, enum açık değeri ve
//! taban tipi: literal, üst düzey `const`, tekli eksi ve tamsayı
//! aritmetiği/kaydırma. SV üreticisi (`volt-sv-emit::structs::const_int`)
//! ve test dili (`volt-hir::sim_port::const_value`, #376) bu fonksiyonu
//! kullanır; önceden iki kopyası vardı ve ayrışmıştı (test dili `%`'yi
//! katlamıyordu, derinlik sınırı farklıydı; #235). Anlamı SV üreticisinin
//! kopyasınınkidir: üretilen SV bu birleştirmeyle değişmez.

use crate::{BinOp, Expr, ExprKind, Idx, ItemKind, SourceFile, UnOp};

/// Değerlendirmenin en çok iç içeliği (savunma; döngüsel `const` E2020).
pub const MAX_CONST_DEPTH: u32 = 32;

/// Sabit ifadenin değeri; sabit değilse ya da taşarsa `None`.
pub fn const_int(ast: &SourceFile, e: Idx<Expr>) -> Option<i128> {
    go(ast, e, 0)
}

fn go(ast: &SourceFile, e: Idx<Expr>, depth: u32) -> Option<i128> {
    if depth > MAX_CONST_DEPTH {
        return None;
    }
    match &ast.exprs[e].kind {
        ExprKind::IntLit { value, .. } => i128::try_from(*value).ok(),
        ExprKind::Path(p) if p.segments.len() == 1 => {
            let name = &p.segments[0].text;
            ast.items.iter().find_map(|&i| {
                let ItemKind::Const(c) = &ast.items_arena[i].kind else {
                    return None;
                };
                if c.name.text != *name {
                    return None;
                }
                go(ast, c.value, depth + 1)
            })
        }
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => go(ast, *operand, depth + 1)?.checked_neg(),
        ExprKind::Binary { op, lhs, rhs } => {
            let (l, r) = (go(ast, *lhs, depth + 1)?, go(ast, *rhs, depth + 1)?);
            match op {
                BinOp::Add => l.checked_add(r),
                BinOp::Sub => l.checked_sub(r),
                BinOp::Mul => l.checked_mul(r),
                BinOp::Div => l.checked_div(r),
                BinOp::Rem => l.checked_rem(r),
                BinOp::Shl => u32::try_from(r).ok().and_then(|r| l.checked_shl(r)),
                BinOp::Shr => u32::try_from(r).ok().and_then(|r| l.checked_shr(r)),
                BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor
                | BinOp::Eq
                | BinOp::Ne
                | BinOp::Lt
                | BinOp::Gt
                | BinOp::Le
                | BinOp::Ge
                | BinOp::And
                | BinOp::Or
                | BinOp::Imp => None,
            }
        }
        ExprKind::BoolLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::Path(_)
        | ExprKind::Unary { .. }
        | ExprKind::Index { .. }
        | ExprKind::Range { .. }
        | ExprKind::PartSelect { .. }
        | ExprKind::Field { .. }
        | ExprKind::Call { .. }
        | ExprKind::Cast { .. }
        | ExprKind::If { .. }
        | ExprKind::Match { .. }
        | ExprKind::StructLit { .. }
        | ExprKind::ArrayLit(_)
        | ExprKind::TupleLit(_)
        | ExprKind::Concat(_)
        | ExprKind::Todo { .. }
        | ExprKind::Error => None,
    }
}

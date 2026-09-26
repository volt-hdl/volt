//! İfade ağacı gezinme yardımcıları.

use crate::{ArrayLitKind, Expr, ExprKind, Idx, MatchArmBody, SourceFile};

/// Bir ifadenin doğrudan alt ifadeleri, kaynak sırasıyla. `match`
/// kolunun blok gövdesi (deyim) dahil değildir; yalnız bekçi ve ifade
/// gövdesi. `StructLit` kısa alanı (`P { a }`) ifade değildir.
pub fn expr_children(kind: &ExprKind) -> Vec<Idx<Expr>> {
    match kind {
        ExprKind::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ExprKind::Unary { operand, .. } => vec![*operand],
        ExprKind::Index { base, index } => vec![*base, *index],
        ExprKind::Range { base, hi, lo } => vec![*base, *hi, *lo],
        ExprKind::PartSelect {
            base, start, width, ..
        } => vec![*base, *start, *width],
        ExprKind::Field { base, .. } => vec![*base],
        ExprKind::Call { callee, args } => {
            let mut v = vec![*callee];
            v.extend(args.iter().copied());
            v
        }
        ExprKind::Cast { expr, .. } => vec![*expr],
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => vec![*cond, *then_expr, *else_expr],
        ExprKind::Match { scrutinee, arms } => {
            let mut v = vec![*scrutinee];
            for arm in arms {
                v.extend(arm.guard);
                if let MatchArmBody::Expr(e) = arm.body {
                    v.push(e);
                }
            }
            v
        }
        ExprKind::StructLit { fields, .. } => fields.iter().filter_map(|f| f.value).collect(),
        ExprKind::ArrayLit(ArrayLitKind::List(items)) | ExprKind::TupleLit(items) => items.clone(),
        ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => vec![*value, *count],
        ExprKind::Concat(items) => items.iter().map(|&(e, _)| e).collect(),
        ExprKind::IntLit { .. }
        | ExprKind::BoolLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::Path(_)
        | ExprKind::Todo { .. }
        | ExprKind::Error => Vec::new(),
    }
}

/// `root` ve bütün alt ifadeleri önce-kök sırasıyla ziyaret eder
/// (yinelemeli — derin ağaçta yığın taşmaz).
pub fn walk_expr(ast: &SourceFile, root: Idx<Expr>, mut f: impl FnMut(Idx<Expr>)) {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        f(e);
        let children = expr_children(&ast.exprs[e].kind);
        stack.extend(children.into_iter().rev());
    }
}

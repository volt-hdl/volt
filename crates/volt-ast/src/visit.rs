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

/// `kind`'in bir kopyası; her doğrudan alt ifade `f`'nin döndürdüğüyle
/// değiştirilir ([`expr_children`] ile aynı küme ve sıra). Kopyalayan ve
/// ikame eden geçitler (ADR-0081 fn açılımı) düğüm biçimini tek yerde
/// bilir.
pub fn map_children(kind: &ExprKind, mut f: impl FnMut(Idx<Expr>) -> Idx<Expr>) -> ExprKind {
    match kind {
        ExprKind::Binary { op, lhs, rhs } => ExprKind::Binary {
            op: *op,
            lhs: f(*lhs),
            rhs: f(*rhs),
        },
        ExprKind::Unary { op, operand } => ExprKind::Unary {
            op: *op,
            operand: f(*operand),
        },
        ExprKind::Index { base, index } => ExprKind::Index {
            base: f(*base),
            index: f(*index),
        },
        ExprKind::Range { base, hi, lo } => ExprKind::Range {
            base: f(*base),
            hi: f(*hi),
            lo: f(*lo),
        },
        ExprKind::PartSelect {
            base,
            start,
            width,
            ascending,
        } => ExprKind::PartSelect {
            base: f(*base),
            start: f(*start),
            width: f(*width),
            ascending: *ascending,
        },
        ExprKind::Field { base, field } => ExprKind::Field {
            base: f(*base),
            field: field.clone(),
        },
        ExprKind::Call { callee, args } => ExprKind::Call {
            callee: f(*callee),
            args: args.iter().map(|&a| f(a)).collect(),
        },
        ExprKind::Cast { expr, ty } => ExprKind::Cast {
            expr: f(*expr),
            ty: *ty,
        },
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => ExprKind::If {
            cond: f(*cond),
            then_expr: f(*then_expr),
            else_expr: f(*else_expr),
        },
        ExprKind::Match { scrutinee, arms } => {
            let scrutinee = f(*scrutinee);
            let arms = arms
                .iter()
                .map(|arm| crate::MatchArm {
                    span: arm.span,
                    pattern: arm.pattern,
                    guard: arm.guard.map(&mut f),
                    body: match arm.body {
                        MatchArmBody::Expr(e) => MatchArmBody::Expr(f(e)),
                        MatchArmBody::Block(b) => MatchArmBody::Block(b),
                    },
                })
                .collect();
            ExprKind::Match { scrutinee, arms }
        }
        ExprKind::StructLit { path, fields } => ExprKind::StructLit {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|fi| crate::FieldInit {
                    span: fi.span,
                    name: fi.name.clone(),
                    value: fi.value.map(&mut f),
                })
                .collect(),
        },
        ExprKind::ArrayLit(ArrayLitKind::List(items)) => {
            ExprKind::ArrayLit(ArrayLitKind::List(items.iter().map(|&i| f(i)).collect()))
        }
        ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
            ExprKind::ArrayLit(ArrayLitKind::Repeat {
                value: f(*value),
                count: f(*count),
            })
        }
        ExprKind::TupleLit(items) => ExprKind::TupleLit(items.iter().map(|&i| f(i)).collect()),
        ExprKind::Concat(items) => {
            ExprKind::Concat(items.iter().map(|&(e, t)| (f(e), t)).collect())
        }
        ExprKind::IntLit { .. }
        | ExprKind::BoolLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::Path(_)
        | ExprKind::Todo { .. }
        | ExprKind::Error => kind.clone(),
    }
}

//! Sentez modu (type-inference.md §3): literal (§3.1), değişken
//! referansı (§3.2), tekli operatör (§3.4) ve koşullu ifade (§3.7).
//! İkili operatörler `binop`, seçimler `select`, tip dönüşümü `cast`
//! modülündedir.

use volt_ast::{ArrayLitKind, Expr, ExprKind, FieldInit, Idx, IntSuffix, ItemKind, MatchArm, UnOp};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use super::width::IntMeet;
use super::TypeChecker;
use crate::consteval::MAX_ARRAY_LEN;
use crate::resolve::{BuiltinKind, DefKind};
use crate::ty::{StructId, Ty, TypeId};

impl TypeChecker<'_, '_> {
    pub(super) fn synth(&mut self, expr: Idx<Expr>) -> TypeId {
        let ty = self.synth_uncached(expr);
        self.expr_types.insert(expr, ty);
        ty
    }

    pub(super) fn synth_uncached(&mut self, expr: Idx<Expr>) -> TypeId {
        let ast = self.ast;
        let span = ast.exprs[expr].span;
        match &ast.exprs[expr].kind {
            ExprKind::IntLit { value, suffix, .. } => self.synth_int_lit(*value, *suffix, span),
            ExprKind::BoolLit(_) => self.types.bool_ty(),
            ExprKind::Path(_) => match self.res.resolutions.get(&expr) {
                Some(&def) => self.def_type(def),
                // Çözülemeyen isim E1001'i zaten aldı.
                None => self.types.error(),
            },
            ExprKind::Binary { op, lhs, rhs } => self.synth_binary(*op, *lhs, *rhs, span),
            ExprKind::Unary { op, operand } => self.synth_unary(*op, *operand, span),
            ExprKind::Index { .. } | ExprKind::Range { .. } | ExprKind::PartSelect { .. } => {
                self.synth_select(expr)
            }
            ExprKind::Field { base, field } => {
                let base_ty = self.synth(*base);
                self.field_result(base_ty, field, span)
            }
            ExprKind::Call { callee, args } => self.synth_call(*callee, args),
            ExprKind::Cast { expr: inner, ty } => self.synth_cast(*inner, *ty, span),
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => self.synth_if(*cond, *then_expr, *else_expr, span),
            ExprKind::Match { scrutinee, arms } => self.synth_match(*scrutinee, arms),
            ExprKind::StructLit { fields, .. } => self.synth_struct_lit(expr, fields),
            ExprKind::ArrayLit(kind) => self.synth_array_lit(kind),
            ExprKind::TupleLit(items) => {
                let tys: Vec<TypeId> = items.iter().map(|&i| self.synth(i)).collect();
                self.types.intern(Ty::Tuple(tys))
            }
            // todo! her tiple uyumludur; string F2a'da tiplenmez.
            ExprKind::Todo { .. } | ExprKind::StringLit(_) | ExprKind::Error => self.types.error(),
        }
    }

    /// §3.1 — sonekli literal kendi tipindedir (sınır denetimiyle),
    /// soneksiz literal bağlam bekler (`IntLit`).
    fn synth_int_lit(&mut self, value: u128, suffix: Option<IntSuffix>, span: Span) -> TypeId {
        match suffix {
            Some(s) => {
                let ty = self.suffix_ty(s);
                self.check_literal_fits(value, ty, span);
                ty
            }
            None => self.types.int_lit(),
        }
    }

    /// §3.5 — indeks/aralık/parça seçimi: taban ve alt ifadeler
    /// sentezlenir, sonuç kuralı `select` modülündedir.
    fn synth_select(&mut self, expr: Idx<Expr>) -> TypeId {
        let ast = self.ast;
        let span = ast.exprs[expr].span;
        match &ast.exprs[expr].kind {
            ExprKind::Index { base, index } => {
                let base_ty = self.synth(*base);
                self.synth(*index);
                self.index_result(base_ty, *index, span)
            }
            ExprKind::Range { base, hi, lo } => {
                let base_ty = self.synth(*base);
                self.range_result(base_ty, *hi, *lo, span)
            }
            ExprKind::PartSelect {
                base,
                start,
                width,
                ascending,
            } => {
                let base_ty = self.synth(*base);
                self.synth(*start);
                self.synth(*width);
                self.part_select_result(base_ty, *start, *width, *ascending, span)
            }
            _ => unreachable!("synth_select yalnız seçim ifadeleriyle çağrılır"),
        }
    }

    /// prev(x[, N]) argümanının tipini taşır (ADR-0040); diğer yerleşik
    /// çağrı tipleri F2b (sync/zext/concat...).
    fn synth_call(&mut self, callee: Idx<Expr>, args: &[Idx<Expr>]) -> TypeId {
        let is_prev = self
            .res
            .resolutions
            .get(&callee)
            .is_some_and(|&d| self.res.def_kind(d) == DefKind::Builtin(BuiltinKind::Prev));
        let mut first = None;
        for &a in args {
            let t = self.synth(a);
            if first.is_none() {
                first = Some(t);
            }
        }
        match (is_prev, first) {
            (true, Some(t)) => t,
            _ => self.types.error(),
        }
    }

    fn synth_array_lit(&mut self, kind: &ArrayLitKind) -> TypeId {
        match kind {
            ArrayLitKind::List(items) => {
                let Some((&first, rest)) = items.split_first() else {
                    return self.types.error();
                };
                let elem = self.synth(first);
                for &i in rest {
                    self.check(i, elem);
                }
                self.types.intern(Ty::Array {
                    elem,
                    len: items.len() as u32,
                })
            }
            ArrayLitKind::Repeat { value, count } => {
                let elem = self.synth(*value);
                match self.try_const_eval(*count) {
                    Some(n) if (0..=MAX_ARRAY_LEN as i128).contains(&n) => {
                        self.types.intern(Ty::Array {
                            elem,
                            len: n as u32,
                        })
                    }
                    _ => self.types.error(),
                }
            }
        }
    }

    /// §3.4 — tekli operatörler.
    fn synth_unary(&mut self, op: UnOp, operand: Idx<Expr>, span: Span) -> TypeId {
        let ot = self.synth(operand);
        match op {
            UnOp::Not => {
                self.check_is_bool(ot, span);
                self.types.bool_ty()
            }
            // Bit tersleme genişliği korur.
            UnOp::BitNot => ot,
            UnOp::Neg => match *self.types.ty(ot) {
                Ty::SInt { width } | Ty::SIntFlex { hi: width, .. } => {
                    self.types.intern(Ty::SInt {
                        width: width.saturating_add(1),
                    })
                }
                Ty::Trit => self.types.intern(Ty::Trit),
                Ty::IntLit => self.types.int_lit(),
                Ty::UInt { .. } | Ty::UIntFlex { .. } => {
                    self.error(
                        ErrorCode::E2002,
                        span,
                        lstr!(en: "cannot negate an unsigned value"; tr: "işaretsiz değer negatiflenemez"),
                        lstr!(en: "signed type required"; tr: "işaretli tip gerekli"),
                        lstr!(en: "convert to a signed type such as i8/i16 first"; tr: "önce i8/i16 gibi işaretli tipe dönüştürün"),
                    );
                    self.types.error()
                }
                _ => self.types.error(),
            },
        }
    }

    fn check_is_bool(&mut self, ty: TypeId, span: Span) {
        if self.types.is_error(ty) || matches!(self.types.ty(ty), Ty::Bool) {
            return;
        }
        let bool_ty = self.types.bool_ty();
        self.err_type_mismatch(bool_ty, ty, span);
    }

    /// §3.7 — koşullu ifade; literal dallar somut dala uyarlanır, kalan
    /// dallar aynı tipte olmalı (E2003, iki tip de mesajda gösterilir).
    fn synth_if(
        &mut self,
        cond: Idx<Expr>,
        then_expr: Idx<Expr>,
        else_expr: Idx<Expr>,
        span: Span,
    ) -> TypeId {
        let bool_ty = self.types.bool_ty();
        self.check(cond, bool_ty);
        let then_ty = self.synth(then_expr);
        let else_ty = self.synth(else_expr);
        if self.types.is_error(then_ty) || self.types.is_error(else_ty) {
            return self.types.error();
        }
        match (
            self.types.is_int_lit(then_ty),
            self.types.is_int_lit(else_ty),
        ) {
            (true, true) => return then_ty,
            (true, false) => {
                self.check(then_expr, else_ty);
                return else_ty;
            }
            (false, true) => {
                self.check(else_expr, then_ty);
                return then_ty;
            }
            (false, false) => {}
        }
        if then_ty == else_ty {
            return then_ty;
        }
        // Esnek aralıklar kesişiyorsa ortak aralık dalların birleşimidir.
        if let IntMeet::Common { signed, lo, hi } = self.meet_int_ranges(then_ty, else_ty) {
            return self.flex(signed, lo, hi);
        }
        let t = self.types.display(then_ty);
        let e = self.types.display(else_ty);
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "if/else branches have different types: '{t}' and '{e}'"; tr: "if/else dalları farklı tipte: '{t}' ile '{e}'"),
            &lstr!(en: "make the branches the same type; convert with as if needed"; tr: "dalları aynı tipe getirin; gerekirse as ile dönüştürün"),
        );
        self.types.error()
    }

    /// Match ifadesi F2a'da tiplenmez; kollar yine denetlenir.
    fn synth_match(&mut self, scrutinee: Idx<Expr>, arms: &[MatchArm]) -> TypeId {
        self.synth(scrutinee);
        for arm in arms {
            self.check_arm(arm);
        }
        self.types.error()
    }

    fn synth_struct_lit(&mut self, expr: Idx<Expr>, fields: &[FieldInit]) -> TypeId {
        let ast = self.ast;
        let Some(&def) = self.res.resolutions.get(&expr) else {
            return self.types.error();
        };
        if self.res.def_kind(def) != DefKind::Struct {
            return self.types.error();
        }
        if let Some(&item_idx) = self.res.item_of_def.get(&def) {
            if let ItemKind::Struct(s) = &ast.items_arena[item_idx].kind {
                for init in fields {
                    let Some(value) = init.value else { continue };
                    if let Some(f) = s.fields.iter().find(|f| f.name.text == init.name.text) {
                        let ty = self.resolve_type_ref(f.ty);
                        self.check(value, ty);
                    } else {
                        self.synth(value);
                    }
                }
            }
        }
        self.types.intern(Ty::Struct(StructId(def.0)))
    }

    fn suffix_ty(&mut self, suffix: IntSuffix) -> TypeId {
        let ty = match suffix {
            IntSuffix::U8 => Ty::UInt { width: 8 },
            IntSuffix::U16 => Ty::UInt { width: 16 },
            IntSuffix::U32 => Ty::UInt { width: 32 },
            IntSuffix::U64 => Ty::UInt { width: 64 },
            IntSuffix::I8 => Ty::SInt { width: 8 },
            IntSuffix::I16 => Ty::SInt { width: 16 },
            IntSuffix::I32 => Ty::SInt { width: 32 },
            IntSuffix::I64 => Ty::SInt { width: 64 },
        };
        self.types.intern(ty)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, def_ty};

    fn module(body: &str) -> String {
        format!(
            "module M {{\n    in  a : u8\n    in  s : i8\n    in  f : bool\n    out y : u8\n\n{body}\n    y = a\n}}\n"
        )
    }

    #[test]
    fn suffixed_literal_has_its_own_type() {
        assert_eq!(def_ty(&module("    let _x = 42u16"), "_x"), "u16");
    }

    #[test]
    fn path_takes_the_declared_type() {
        assert_eq!(def_ty(&module("    let _x = a"), "_x"), "u8");
    }

    #[test]
    fn negation_widens_signed_and_rejects_unsigned() {
        assert_eq!(def_ty(&module("    let _n = -s"), "_n"), "i9");
        assert!(codes(&module("    let _n = -a")).contains(&"E2002"));
    }

    #[test]
    fn if_expression_adapts_literal_branch_and_rejects_mixed_branches() {
        assert_eq!(
            def_ty(&module("    let _i = if f { a } else { 0 }"), "_i"),
            "u8"
        );
        assert!(codes(&module("    let _i = if f { a } else { f }")).contains(&"E2003"));
    }
}

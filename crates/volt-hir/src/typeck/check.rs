//! Kontrol modu (type-inference.md §4) ve atanabilirlik (§5).
//!
//! ADR-0041: hedef tip açıkça yazılmışsa (check modu) aynı işaretli
//! genişleme örtüktür — beklenen tip aritmetik operandlara itilir
//! (`let c : i19 = a + b` önce a ve b'yi i19'a genişletir). Daraltma
//! (E2001) ve işaret farkı (E2002) her iki modda da hata kalır.

use volt_ast::{ArrayLitKind, BinOp, Expr, ExprKind, Idx};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use super::width::{sign_prefix, sint_fits, uint_fits};
use super::TypeChecker;
use crate::ty::{Ty, TypeId};

impl TypeChecker<'_, '_> {
    pub(super) fn check(&mut self, expr: Idx<Expr>, expected: TypeId) {
        // Error her tiple uyumlu — ama alt ifadeler yine denetlenir.
        if self.types.is_error(expected) {
            self.synth(expr);
            return;
        }
        let ast = self.ast;
        let span = ast.exprs[expr].span;
        match &ast.exprs[expr].kind {
            ExprKind::IntLit {
                value,
                suffix: None,
                ..
            } => {
                self.check_int_lit(*value, expected, span);
                self.expr_types.insert(expr, expected);
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let (cond, then_expr, else_expr) = (*cond, *then_expr, *else_expr);
                let bool_ty = self.types.bool_ty();
                self.check(cond, bool_ty);
                self.check(then_expr, expected);
                self.check(else_expr, expected);
                self.expr_types.insert(expr, expected);
            }
            // ADR-0041: beklenen somut uN/iN aritmetik operandlara itilir.
            ExprKind::Binary { op, lhs, rhs }
                if matches!(
                    op,
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem
                ) =>
            {
                let (op, lhs, rhs) = (*op, *lhs, *rhs);
                self.check_arith(expr, op, lhs, rhs, expected, span);
            }
            ExprKind::ArrayLit(kind) if matches!(*self.types.ty(expected), Ty::Array { .. }) => {
                self.check_array_lit(expr, kind, expected, span);
            }
            _ => {
                let actual = self.synth(expr);
                self.expect_assignable(actual, expected, span);
            }
        }
    }

    /// ADR-0035: dizi literalleri hedef eleman tipine daraltılır —
    /// `reg regs : [u32; 32] = [0; 32]` içindeki 0 bir u32'dir.
    fn check_array_lit(
        &mut self,
        expr: Idx<Expr>,
        kind: &ArrayLitKind,
        expected: TypeId,
        span: Span,
    ) {
        let Ty::Array { elem, len } = *self.types.ty(expected) else {
            unreachable!("çağıran beklenen tipin dizi olduğunu denetler")
        };
        match kind {
            ArrayLitKind::Repeat { value, count } => {
                self.check(*value, elem);
                match self.try_const_eval(*count) {
                    Some(n) if n == i128::from(len) => {}
                    _ => {
                        let actual = self.synth_uncached(expr);
                        self.expect_assignable(actual, expected, span);
                    }
                }
            }
            ArrayLitKind::List(items) => {
                for &i in items {
                    self.check(i, elem);
                }
                if items.len() as u32 != len {
                    let actual = self.types.intern(Ty::Array {
                        elem,
                        len: items.len() as u32,
                    });
                    self.err_type_mismatch(expected, actual, span);
                }
            }
        }
        self.expr_types.insert(expr, expected);
    }

    /// ADR-0041 — check modunda aritmetik: hedef tip açıkça yazılmışsa
    /// aynı işaretli, hedefe sığan operandlar ÖNCE hedef genişliğe
    /// genişletilir, işlem sonra yapılır (`let c : i19 = a + b`). Uygun
    /// olmayan durumlar (Trit, bits, yalnız literal, işaret farkı,
    /// hedeften geniş operand) sentez yoluna düşer; sonuç yine
    /// atanabilirlik denetiminden geçer (daraltma E2001 kalır).
    fn check_arith(
        &mut self,
        expr: Idx<Expr>,
        op: BinOp,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        expected: TypeId,
        span: Span,
    ) {
        let lt = self.synth(lhs);
        let rt = self.synth(rhs);
        let actual = if self.widen_operands(lhs, rhs, lt, rt, expected) {
            // Genişletilmiş operandların sonucu (lo = hedef) her zaman sığar.
            let result = self.arith_result(op, expected, expected, span);
            self.expect_assignable(result, expected, span);
            expected
        } else {
            self.arith_of(op, lhs, rhs, lt, rt, span)
        };
        self.expr_types.insert(expr, actual);
        if actual != expected {
            self.expect_assignable(actual, expected, span);
        }
    }

    /// Genişleme uygunluğu (ADR-0041): beklenen somut uN/iN; en az bir
    /// operand literal değil; literal olmayan her operand aynı işaretli
    /// ve doğal genişliği hedefe sığıyor. Uygunsa literaller hedef tipe
    /// uyarlanır ve operandların kayıtlı tipi genişletilmiş hedef olur.
    fn widen_operands(
        &mut self,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        lt: TypeId,
        rt: TypeId,
        expected: TypeId,
    ) -> bool {
        if !matches!(self.types.ty(expected), Ty::UInt { .. } | Ty::SInt { .. }) {
            return false;
        }
        let Some((sign, _, width)) = self.types.int_range(expected) else {
            return false;
        };
        let operands = [(lhs, lt), (rhs, rt)];
        if operands.iter().all(|&(_, t)| self.types.is_int_lit(t)) {
            return false;
        }
        for &(_, t) in &operands {
            if self.types.is_int_lit(t) {
                continue;
            }
            match self.types.int_range(t) {
                Some((s, _, hi)) if s == sign && hi <= width => {}
                _ => return false,
            }
        }
        for (e, t) in operands {
            if self.types.is_int_lit(t) {
                self.check(e, expected); // sınır denetimi + tip kaydı
            } else {
                self.expr_types.insert(e, expected);
            }
        }
        true
    }

    /// Soneksiz literali beklenen tipe uyarla (§4).
    fn check_int_lit(&mut self, value: u128, expected: TypeId, span: Span) {
        match *self.types.ty(expected) {
            Ty::UInt { width } | Ty::UIntFlex { hi: width, .. } => {
                if !uint_fits(value, width) {
                    self.literal_overflow(value, expected, span);
                }
            }
            Ty::SInt { width } | Ty::SIntFlex { hi: width, .. } => {
                if !sint_fits(value, width) {
                    self.literal_overflow(value, expected, span);
                }
            }
            Ty::Trit => {
                if !matches!(value, 0 | 1) {
                    self.error(
                        ErrorCode::E2011,
                        span,
                        lstr!(en: "Trit literal must be {{-1, 0, +1}}"; tr: "Trit literali {{-1, 0, +1}} olmalı"),
                        lstr!(en: "{value} is not in this set"; tr: "{value} bu kümede değil"),
                        lstr!(en: "make the value -1, 0 or 1"; tr: "değeri -1, 0 veya 1 yapın"),
                    );
                }
            }
            Ty::Bool => {
                self.error(
                    ErrorCode::E2003,
                    span,
                    lstr!(en: "numeric literal in bool context"; tr: "sayısal literal bool bağlamında"),
                    lstr!(en: "expected bool"; tr: "bool bekleniyor"),
                    lstr!(en: "write true or false"; tr: "true veya false yazın"),
                );
            }
            _ => {
                let lit = self.types.int_lit();
                self.err_type_mismatch(expected, lit, span);
            }
        }
    }

    /// Sonekli literal kendi tipine sığmalı (§3.1, E2010).
    pub(super) fn check_literal_fits(&mut self, value: u128, ty: TypeId, span: Span) {
        let fits = match *self.types.ty(ty) {
            Ty::UInt { width } => uint_fits(value, width),
            Ty::SInt { width } => sint_fits(value, width),
            _ => true,
        };
        if !fits {
            self.literal_overflow(value, ty, span);
        }
    }

    /// §5 — atanabilirlik: örtük daraltma yasak (E2001), işaret farkı
    /// E2002. Esnek aritmetik sonucu (ADR-0025) hedef genişliği
    /// aralığındaysa uyar. ADR-0041: beklenen tip check moduna yalnız
    /// açık bildirimlerden (let/reg/wire/port/const tipi, atama hedefi,
    /// port bağlama) girdiğinden aynı işaretli genişleme burada örtüktür.
    pub(super) fn expect_assignable(&mut self, actual: TypeId, expected: TypeId, span: Span) {
        if actual == expected || self.types.is_error(actual) || self.types.is_error(expected) {
            return;
        }
        // Literal her sayısal tipe uyar (sınır kontrolü yapıldı).
        if self.types.is_int_lit(actual) && self.is_literal_adaptable(expected) {
            return;
        }
        if let (Some((sa, alo, ahi)), Some((se, elo, ehi))) =
            (self.types.int_range(actual), self.types.int_range(expected))
        {
            if sa != se {
                self.error(
                    ErrorCode::E2002,
                    span,
                    lstr!(en: "sign mismatch"; tr: "işaret uyumsuzluğu"),
                    lstr!(en: "signed and unsigned are mixed"; tr: "işaretli ve işaretsiz karışıyor"),
                    lstr!(en: "use an explicit cast with as"; tr: "as ile açık dönüşüm yapın"),
                );
                return;
            }
            // Kesişim (esnek sonuç) ya da hedefe sığan doğal genişlik
            // (ADR-0041 genişleme) — ikisi de kabul.
            if alo.max(elo) <= ahi.min(ehi) || ahi <= ehi {
                return;
            }
            self.width_mismatch(alo, ehi, sign_prefix(sa), span);
            return;
        }
        match (self.types.ty(actual), self.types.ty(expected)) {
            (&Ty::Bits { width: a }, &Ty::Bits { width: b }) => {
                self.error(
                    ErrorCode::E2001,
                    span,
                    lstr!(
                        en: "bit width mismatch: a bits<{a}> value cannot be assigned to a bits<{b}> target";
                        tr: "bit genişliği uyumsuzluğu: bits<{a}> değeri bits<{b}> hedefe atanamaz"
                    ),
                    lstr!(en: "widths differ"; tr: "genişlikler farklı"),
                    lstr!(en: "make the source and target widths equal"; tr: "kaynak ve hedef genişliklerini eşitleyin"),
                );
            }
            _ => self.err_type_mismatch(expected, actual, span),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, def_ty, expr_type_names};

    fn module(body: &str) -> String {
        format!(
            "module M {{\n    in  a : u8\n    in  b : u8\n    in  s : i8\n    out y : u8\n\n{body}\n    y = a\n}}\n"
        )
    }

    #[test]
    fn unsuffixed_literal_takes_the_expected_type() {
        let src = module("    let _x : u16 = 1000");
        assert!(codes(&src).is_empty(), "{:?}", codes(&src));
        assert!(expr_type_names(&src).contains(&"u16".to_string()));
    }

    #[test]
    fn literal_outside_expected_range_is_e2010() {
        assert!(codes(&module("    let _x : u8 = 256")).contains(&"E2010"));
    }

    #[test]
    fn explicit_wider_target_widens_operands_first() {
        let src = module("    let _w : u16 = a * b");
        assert!(codes(&src).is_empty(), "{:?}", codes(&src));
        assert_eq!(def_ty(&src, "_w"), "u16");
    }

    #[test]
    fn implicit_narrowing_and_sign_change_are_rejected() {
        assert!(codes(&module("    let _n : u4 = a")).contains(&"E2001"));
        assert!(codes(&module("    let _n : u8 = s")).contains(&"E2002"));
    }

    #[test]
    fn array_literal_elements_adapt_to_the_element_type() {
        let ok = module("    let _t : [u8; 2] = [1, 2]");
        assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
        assert!(codes(&module("    let _t : [u8; 3] = [1, 2]")).contains(&"E2003"));
    }
}

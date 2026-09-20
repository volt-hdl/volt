//! İkili operatörler (type-inference.md §3.3): aritmetik, bit düzeyi,
//! kaydırma, karşılaştırma, mantıksal.
//!
//! Aritmetik sonuçlar esnek genişlik aralığı taşır (`Ty::UIntFlex`/
//! `SIntFlex`, ADR-0025): `u8 + u8` doğal olarak `u9`'dur ama sayaç
//! deseni (`count <= count + 1`) taşma bitini atarak operand
//! genişliğine de uyarlanabilir.

use volt_ast::{BinOp, Expr, Idx};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use super::width::{clamp_width, IntMeet};
use super::TypeChecker;
use crate::ty::{Ty, TypeId};

impl TypeChecker<'_, '_> {
    pub(super) fn synth_binary(
        &mut self,
        op: BinOp,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        span: Span,
    ) -> TypeId {
        use BinOp::{
            Add, And, BitAnd, BitOr, BitXor, Div, Eq, Ge, Gt, Imp, Le, Lt, Mul, Ne, Or, Rem, Shl,
            Shr, Sub,
        };
        match op {
            Add | Sub | Mul | Div | Rem => self.synth_arith(op, lhs, rhs, span),
            BitAnd | BitOr | BitXor => self.synth_bitwise(lhs, rhs, span),
            Shl | Shr => self.synth_shift(lhs, rhs, span),
            Eq | Ne | Lt | Gt | Le | Ge => self.synth_comparison(lhs, rhs, span),
            // a -> b ≡ !a || b: iki operand da Bool, sonuç Bool (ADR-0034).
            And | Or | Imp => self.synth_logical(lhs, rhs),
        }
    }

    /// Aritmetik (§3.3): taşma genişlemesi. Sonuç, operand genişliği ile
    /// genişlemiş doğal genişlik arasında esnektir (ADR-0025); sayaç
    /// deseni (`count <= count + 1`) böylece taşma bitini atabilir.
    fn synth_arith(&mut self, op: BinOp, lhs: Idx<Expr>, rhs: Idx<Expr>, span: Span) -> TypeId {
        let lt = self.synth(lhs);
        let rt = self.synth(rhs);
        self.arith_of(op, lhs, rhs, lt, rt, span)
    }

    /// Aritmetik sonuç, operand tipleri ZATEN sentezlenmişken — check
    /// modundaki genişleme yolu (`check_arith`) operandları bir kez
    /// sentezler; tanıların çiftlenmemesi için sentez burada tekrarlanmaz.
    pub(super) fn arith_of(
        &mut self,
        op: BinOp,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        lt: TypeId,
        rt: TypeId,
        span: Span,
    ) -> TypeId {
        if self.types.is_error(lt) || self.types.is_error(rt) {
            return self.types.error();
        }
        // bits<N> aritmetiği her kombinasyonda yasak — literalden önce.
        if self.is_bits(lt) || self.is_bits(rt) {
            self.error(
                ErrorCode::E2004,
                span,
                lstr!(en: "cannot perform arithmetic on bits<N>"; tr: "bits<N> tipinde aritmetik yapılamaz"),
                lstr!(en: "bits is a raw bit vector, not a number"; tr: "bits ham bit vektörüdür, sayısal değil"),
                lstr!(en: "convert to a numeric type such as u8/i8"; tr: "u8/i8 gibi sayısal tipe dönüştürün"),
            );
            return self.types.error();
        }
        match (self.types.is_int_lit(lt), self.types.is_int_lit(rt)) {
            (true, true) => return self.types.int_lit(),
            // Literal somut tarafa uyarlanır (sınır denetimiyle).
            (true, false) => {
                if !self.adapt_literal_operand(lhs, rt) {
                    return self.err_arith_incompatible(lt, rt, span);
                }
                return self.arith_result(op, rt, rt, span);
            }
            (false, true) => {
                if !self.adapt_literal_operand(rhs, lt) {
                    return self.err_arith_incompatible(lt, rt, span);
                }
                return self.arith_result(op, lt, lt, span);
            }
            (false, false) => {}
        }
        self.arith_result(op, lt, rt, span)
    }

    pub(super) fn arith_result(&mut self, op: BinOp, lt: TypeId, rt: TypeId, span: Span) -> TypeId {
        let (signed, lo, hi) = match self.operand_range(lt, rt, span) {
            Ok(Some(range)) => range,
            Ok(None) => return self.trit_arith_result(op, lt, rt, span),
            Err(error) => return error,
        };
        // Toplama/çıkarma 1 bit, çarpma genişlik kadar genişler;
        // bölme/mod genişlemez. Sonuç MAX_WIDTH ile sınırlı.
        let natural = match op {
            BinOp::Add | BinOp::Sub => clamp_width(u32::from(hi) + 1),
            BinOp::Mul => clamp_width(u32::from(hi) * 2),
            _ => hi,
        };
        self.flex(signed, lo, natural)
    }

    /// Trit kuralları (§3.3): çarpım kapalı ({-1,0,1} içinde kalır),
    /// toplam/fark i3'e taşar, ternary MAC deseninde işaretli genişlik
    /// korunur; kalan kombinasyonlar E2003.
    fn trit_arith_result(&mut self, op: BinOp, lt: TypeId, rt: TypeId, span: Span) -> TypeId {
        let l_trit = matches!(self.types.ty(lt), Ty::Trit);
        let r_trit = matches!(self.types.ty(rt), Ty::Trit);
        match (l_trit, r_trit) {
            (true, true) => match op {
                BinOp::Mul => self.types.intern(Ty::Trit),
                // +1 + +1 = +2 kümeden çıkar → i3'e genişle.
                BinOp::Add | BinOp::Sub => self.types.intern(Ty::SInt { width: 3 }),
                _ => {
                    self.err_type_mismatch_msg(
                        span,
                        &lstr!(en: "this operator is not defined for Trit"; tr: "bu operatör Trit tipinde tanımlı değil"),
                        &lstr!(en: "Trit only supports *, + and -"; tr: "Trit yalnız *, + ve - destekler"),
                    );
                    self.types.error()
                }
            },
            (true, false) | (false, true) => {
                let other = if l_trit { rt } else { lt };
                if op == BinOp::Mul {
                    if let Some((true, _, _)) = self.types.int_range(other) {
                        return other;
                    }
                }
                let shown = self.types.display(other);
                self.err_type_mismatch_msg(
                    span,
                    &lstr!(en: "this operation is not defined between Trit and '{shown}'"; tr: "Trit ile '{shown}' arasında bu işlem tanımlı değil"),
                    &lstr!(en: "Trit can only be multiplied with a signed type (iN); convert with as if needed"; tr: "Trit yalnız işaretli tiple (iN) çarpılabilir; gerekirse as ile dönüştürün"),
                );
                self.types.error()
            }
            (false, false) => self.err_arith_incompatible(lt, rt, span),
        }
    }

    /// Bit düzeyi (§3.3): genişlemez; aynı genişlik zorunlu.
    fn synth_bitwise(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>, span: Span) -> TypeId {
        let lt = self.synth(lhs);
        let rt = self.synth(rhs);
        if self.types.is_error(lt) || self.types.is_error(rt) {
            return self.types.error();
        }
        match (self.types.is_int_lit(lt), self.types.is_int_lit(rt)) {
            (true, true) => return self.types.int_lit(),
            (true, false) => return self.bitwise_with_literal(lhs, rt, lt, rt, span),
            (false, true) => return self.bitwise_with_literal(rhs, lt, lt, rt, span),
            (false, false) => {}
        }
        match (self.types.ty(lt), self.types.ty(rt)) {
            (Ty::Bool, Ty::Bool) => self.types.bool_ty(),
            (&Ty::Bits { width: a }, &Ty::Bits { width: b }) => {
                if a == b {
                    return lt;
                }
                self.error(
                    ErrorCode::E2001,
                    span,
                    lstr!(en: "bit width mismatch: bits<{a}> and bits<{b}>"; tr: "bit genişliği uyumsuzluğu: bits<{a}> ve bits<{b}>"),
                    lstr!(en: "operand widths differ"; tr: "operand genişlikleri farklı"),
                    lstr!(en: "make the operand widths equal"; tr: "operand genişliklerini eşitleyin"),
                );
                self.types.error()
            }
            _ => match self.operand_range(lt, rt, span) {
                // GENİŞLEMEZ: ortak aralık aynen korunur.
                Ok(Some((signed, lo, hi))) => self.flex(signed, lo, hi),
                Ok(None) => self.err_bitwise_incompatible(lt, rt, span),
                Err(error) => error,
            },
        }
    }

    /// Bit düzeyi işlemde literal operand: karşı taraf tam sayıysa ona
    /// uyarlanır (sınır denetimiyle), değilse E2003.
    fn bitwise_with_literal(
        &mut self,
        literal: Idx<Expr>,
        target: TypeId,
        lt: TypeId,
        rt: TypeId,
        span: Span,
    ) -> TypeId {
        if self.types.int_range(target).is_some() {
            self.check(literal, target);
            target
        } else {
            self.err_bitwise_incompatible(lt, rt, span)
        }
    }

    /// Kaydırma (§3.3): sonuç sol operandın tipi, genişlemez. Sabit
    /// miktar sol genişliğe eşit ya da büyükse W2013.
    fn synth_shift(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>, span: Span) -> TypeId {
        let lt = self.synth(lhs);
        let rt = self.synth(rhs);
        if self.types.is_error(lt) {
            return self.types.error();
        }
        let lhs_ok = self.types.int_range(lt).is_some()
            || matches!(self.types.ty(lt), Ty::Bits { .. } | Ty::IntLit);
        if !lhs_ok {
            let shown = self.types.display(lt);
            self.err_type_mismatch_msg(
                span,
                &lstr!(en: "type '{shown}' cannot be shifted"; tr: "'{shown}' tipi kaydırılamaz"),
                &lstr!(en: "shifts are only defined for uN, iN and bits<N>"; tr: "kaydırma yalnız uN, iN ve bits<N> tiplerinde tanımlı"),
            );
            return self.types.error();
        }
        let rhs_ok = self.types.is_error(rt)
            || self.types.is_int_lit(rt)
            || self.types.int_range(rt).is_some();
        if !rhs_ok {
            let shown = self.types.display(rt);
            self.err_type_mismatch_msg(
                span,
                &lstr!(en: "shift amount must be numeric, found '{shown}'"; tr: "kaydırma miktarı sayısal olmalı, '{shown}' bulundu"),
                &lstr!(en: "provide the amount as uN/iN or as a constant"; tr: "miktarı uN/iN tipinde ya da sabit olarak verin"),
            );
            // Spec: sonuç yine sol operandın tipidir.
            return lt;
        }
        if let (Some(width), Some(amount)) = (self.types.width_of(lt), self.try_const_eval(rhs)) {
            if amount >= i128::from(width) {
                self.warning(
                    ErrorCode::W2013,
                    span,
                    lstr!(en: "shift amount {amount} exceeds the width of {width} bits"; tr: "kaydırma miktarı {amount}, {width} bit genişliği aşıyor"),
                    lstr!(en: "all bits are shifted out, the result is always 0"; tr: "tüm bitler dışarı kayar, sonuç hep 0"),
                    lstr!(en: "keep the amount in the range 0..{width}"; tr: "miktarı 0..{width} aralığında tutun"),
                );
            }
        }
        lt
    }

    /// Karşılaştırma (§3.3): sonuç her zaman Bool; operandlar aynı tipe
    /// birleştirilmeli, uyumsuzluk E2003.
    fn synth_comparison(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>, span: Span) -> TypeId {
        let lt = self.synth(lhs);
        let rt = self.synth(rhs);
        self.unify_for_comparison(lhs, rhs, lt, rt, span);
        self.types.bool_ty()
    }

    fn unify_for_comparison(
        &mut self,
        lhs: Idx<Expr>,
        rhs: Idx<Expr>,
        lt: TypeId,
        rt: TypeId,
        span: Span,
    ) {
        if lt == rt || self.types.is_error(lt) || self.types.is_error(rt) {
            return;
        }
        // Literal karşı tarafın tipine uyarlanır.
        if self.types.is_int_lit(lt) && self.is_literal_adaptable(rt) {
            self.check(lhs, rt);
            return;
        }
        if self.types.is_int_lit(rt) && self.is_literal_adaptable(lt) {
            self.check(rhs, lt);
            return;
        }
        if matches!(self.meet_int_ranges(lt, rt), IntMeet::Common { .. }) {
            return;
        }
        let l = self.types.display(lt);
        let r = self.types.display(rt);
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "comparison operands must have the same type: '{l}' and '{r}'"; tr: "karşılaştırma operandları aynı tipte olmalı: '{l}' ile '{r}'"),
            &lstr!(en: "convert the operands to the same type with as"; tr: "operandları as ile aynı tipe getirin"),
        );
    }

    /// Mantıksal (§3.3): iki operand da Bool, sonuç Bool.
    fn synth_logical(&mut self, lhs: Idx<Expr>, rhs: Idx<Expr>) -> TypeId {
        let bool_ty = self.types.bool_ty();
        self.check(lhs, bool_ty);
        self.check(rhs, bool_ty);
        bool_ty
    }

    // ═══ İkili operatör yardımcıları ══════════════════════════════

    /// Literal operandı somut sayısal tipe uyarlar; hedef sayısal
    /// değilse false döner (çağıran uyumsuzluk hatası verir).
    fn adapt_literal_operand(&mut self, expr: Idx<Expr>, target: TypeId) -> bool {
        let ok = self.is_literal_adaptable(target);
        if ok {
            self.check(expr, target);
        }
        ok
    }

    fn err_arith_incompatible(&mut self, lt: TypeId, rt: TypeId, span: Span) -> TypeId {
        let l = self.types.display(lt);
        let r = self.types.display(rt);
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "incompatible arithmetic operands: '{l}' and '{r}'"; tr: "aritmetik operandları uyumsuz: '{l}' ile '{r}'"),
            &lstr!(en: "convert the operands to the same numeric type"; tr: "operandları aynı sayısal tipe getirin"),
        );
        self.types.error()
    }

    fn err_bitwise_incompatible(&mut self, lt: TypeId, rt: TypeId, span: Span) -> TypeId {
        let l = self.types.display(lt);
        let r = self.types.display(rt);
        self.err_type_mismatch_msg(
            span,
            &lstr!(en: "bitwise operator is not defined for '{l}' and '{r}'"; tr: "bit düzeyi operatör '{l}' ile '{r}' tipinde tanımlı değil"),
            &lstr!(en: "bitwise operations require bool, uN, iN or bits<N>"; tr: "bit düzeyi işlemler bool, uN, iN ve bits<N> ister"),
        );
        self.types.error()
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, def_ty};

    const PORTS: &str =
        "    in  a : u8\n    in  b : u8\n    in  s : i8\n    in  w : bits<8>\n    out y : u8\n";

    fn module(body: &str) -> String {
        format!("module M {{\n{PORTS}\n{body}\n    y = a\n}}\n")
    }

    #[test]
    fn addition_widens_by_one_bit_and_multiplication_doubles() {
        let src = module("    let _sum = a + b\n    let _prod = a * b");
        assert_eq!(def_ty(&src, "_sum"), "u9");
        assert_eq!(def_ty(&src, "_prod"), "u16");
    }

    #[test]
    fn bitwise_keeps_operand_width() {
        assert_eq!(def_ty(&module("    let _and = a & b"), "_and"), "u8");
    }

    #[test]
    fn mixed_signs_are_e2002_in_arithmetic_and_bitwise() {
        assert!(codes(&module("    let _x = a + s")).contains(&"E2002"));
        assert!(codes(&module("    let _x = a | s")).contains(&"E2002"));
    }

    #[test]
    fn arithmetic_on_bits_is_e2004() {
        assert!(codes(&module("    let _x = w + w")).contains(&"E2004"));
    }

    #[test]
    fn shift_keeps_left_type_and_warns_when_amount_exceeds_width() {
        let src = module("    let _sh = a << 9");
        assert_eq!(def_ty(&src, "_sh"), "u8");
        assert!(codes(&src).contains(&"W2013"));
    }

    #[test]
    fn comparison_is_bool_and_rejects_unrelated_types() {
        assert_eq!(def_ty(&module("    let _c = a < b"), "_c"), "bool");
        assert!(codes(&module("    let _c = a == w")).contains(&"E2003"));
    }
}

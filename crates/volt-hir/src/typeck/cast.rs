//! Tip dönüşümü `as` (type-inference.md §3.6): genişletme serbest,
//! daraltma uyarılı (W2010), tanımsız dönüşüm E2009.

use volt_ast::{Expr, Idx, TypeRef};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use super::TypeChecker;
use crate::ty::{Ty, TypeId};

impl TypeChecker<'_, '_> {
    pub(super) fn synth_cast(&mut self, inner: Idx<Expr>, ty: Idx<TypeRef>, span: Span) -> TypeId {
        let src = self.synth(inner);
        let dst = self.resolve_type_ref(ty);
        self.check_cast_legal(src, dst, span);
        dst
    }

    fn check_cast_legal(&mut self, src: TypeId, dst: TypeId, span: Span) {
        // Esnek aritmetik sonucu doğal genişliğiyle dönüştürülür.
        let src = self.types.concrete(src);
        if src == dst || self.types.is_error(src) || self.types.is_error(dst) {
            return;
        }
        if self.check_enum_cast(src, dst, span) {
            return;
        }
        let legal = match (self.types.ty(src).clone(), self.types.ty(dst).clone()) {
            // Literal açık dönüşümle her sayısal tipe gider.
            (Ty::IntLit, Ty::UInt { .. } | Ty::SInt { .. } | Ty::Bits { .. }) => true,
            // Genişletme — her zaman güvenli.
            (Ty::UInt { width: a }, Ty::UInt { width: b }) if b >= a => true,
            (Ty::SInt { width: a }, Ty::SInt { width: b }) if b >= a => true,
            // Daraltma — izinli ama uyarı (W2010).
            (Ty::UInt { width: a }, Ty::UInt { width: b })
            | (Ty::SInt { width: a }, Ty::SInt { width: b }) => {
                self.warning(
                    ErrorCode::W2010,
                    span,
                    lstr!(en: "{a}-bit → {b}-bit narrowing, upper bits are truncated"; tr: "{a} bit → {b} bit daraltma, üst bitler kesilir"),
                    lstr!(en: "possible loss of information"; tr: "bilgi kaybı olabilir"),
                    lstr!(en: "if the narrowing is intentional this is fine; otherwise mask first"; tr: "bilinçli daraltma ise sorun yok; değilse önce maskeleme yapın"),
                );
                true
            }
            // Bool ↔ 1-bit.
            (Ty::Bool, Ty::UInt { width: 1 }) | (Ty::UInt { width: 1 }, Ty::Bool) => true,
            // İşaret değişimi — açık cast ile serbest.
            (Ty::UInt { .. }, Ty::SInt { .. }) | (Ty::SInt { .. }, Ty::UInt { .. }) => true,
            // bits<N> ↔ sayısal, aynı genişlikte.
            (Ty::Bits { width: a }, Ty::UInt { width: b })
            | (Ty::UInt { width: a }, Ty::Bits { width: b })
            | (Ty::Bits { width: a }, Ty::SInt { width: b }) => a == b,
            // Trit → işaretli (genişleme, en az 2 bit).
            (Ty::Trit, Ty::SInt { width }) => width >= 2,
            // Sayısal → Trit YASAK: sessiz kırpma olur.
            _ => false,
        };
        if !legal {
            let src_s = self.show(src);
            let dst_s = self.show(dst);
            self.error(
                ErrorCode::E2009,
                span,
                lstr!(en: "cast '{src_s}' → '{dst_s}' is invalid"; tr: "'{src_s}' → '{dst_s}' dönüşümü geçersiz"),
                lstr!(en: "this cast is not defined"; tr: "bu dönüşüm tanımlı değil"),
                lstr!(en: "an intermediate cast may be needed"; tr: "ara dönüşüm gerekebilir"),
            );
        }
    }
}

impl TypeChecker<'_, '_> {
    /// Enum dönüşümleri (ADR-0074 Karar 3): `enum as uN/bits<N>` yalnız
    /// `N ≥ W` (sıfır genişletme, bilgi kaybı yok); `uN as Enum` yasak —
    /// geçersiz kod üretebilir, çözme `match` ile açık yazılır. Enum
    /// tarafı yoksa `false` (genel kurallar).
    fn check_enum_cast(&mut self, src: TypeId, dst: TypeId, span: Span) -> bool {
        match (self.types.ty(src).clone(), self.types.ty(dst).clone()) {
            (Ty::Enum(e), Ty::UInt { width } | Ty::Bits { width }) => {
                let Some(w) = self.types.enum_width(e) else {
                    return true; // geçersiz kodlama E2030'u aldı
                };
                if width < w {
                    let (src_s, dst_s) = (self.show(src), self.show(dst));
                    self.error(
                        ErrorCode::E2009,
                        span,
                        lstr!(en: "cast '{src_s}' → '{dst_s}' loses information: the enum is {w} bits wide"; tr: "'{src_s}' → '{dst_s}' dönüşümü bilgi kaybeder: enum {w} bit genişliğinde"),
                        lstr!(en: "target narrower than the encoding"; tr: "hedef kodlamadan dar"),
                        lstr!(en: "cast to at least {w} bits: {src_s} as u{w}"; tr: "en az {w} bite dönüştürün: {src_s} as u{w}"),
                    );
                }
                true
            }
            (_, Ty::Enum(_)) => {
                let (src_s, dst_s) = (self.show(src), self.show(dst));
                self.error(
                    ErrorCode::E2009,
                    span,
                    lstr!(en: "cast '{src_s}' → '{dst_s}' is invalid: a number may hold a code that is no variant of '{dst_s}'"; tr: "'{src_s}' → '{dst_s}' dönüşümü geçersiz: sayı '{dst_s}' enum'unun hiçbir varyantı olmayan bir kod taşıyabilir"),
                    lstr!(en: "no implicit decoding into an enum"; tr: "enum'a örtük çözme yok"),
                    lstr!(en: "decode explicitly with a match and choose what invalid codes become: match raw {{ 0 => {{ s = {dst_s}::A }} ... _ => {{ s = {dst_s}::A }} }}"; tr: "match ile açıkça çözün ve geçersiz kodların ne olacağını seçin: match raw {{ 0 => {{ s = {dst_s}::A }} ... _ => {{ s = {dst_s}::A }} }}"),
                );
                true
            }
            (Ty::Enum(_), _) => {
                let (src_s, dst_s) = (self.show(src), self.show(dst));
                self.error(
                    ErrorCode::E2009,
                    span,
                    lstr!(en: "cast '{src_s}' → '{dst_s}' is invalid"; tr: "'{src_s}' → '{dst_s}' dönüşümü geçersiz"),
                    lstr!(en: "this cast is not defined"; tr: "bu dönüşüm tanımlı değil"),
                    lstr!(en: "an enum converts only to an unsigned uN or bits<N> at least as wide as its encoding"; tr: "enum yalnız kodlaması kadar geniş işaretsiz uN ya da bits<N>'e dönüşür"),
                );
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, def_ty};

    fn module(body: &str) -> String {
        format!(
            "module M {{\n    in  a : u8\n    in  f : bool\n    in  t : Trit\n    out y : u8\n\n{body}\n    y = a\n}}\n"
        )
    }

    #[test]
    fn narrowing_cast_warns_and_illegal_cast_is_e2009() {
        let narrowing = module("    let _c = a as u4");
        assert_eq!(def_ty(&narrowing, "_c"), "u4");
        assert!(codes(&narrowing).contains(&"W2010"));
        assert!(codes(&module("    let _c = f as u8")).contains(&"E2009"));
    }

    #[test]
    fn widening_and_sign_change_casts_are_silent() {
        let src = module("    let _w = a as u16\n    let _s = a as i8");
        assert!(codes(&src).is_empty(), "{:?}", codes(&src));
        assert_eq!(def_ty(&src, "_w"), "u16");
    }

    #[test]
    fn trit_widens_to_signed_but_numeric_to_trit_is_forbidden() {
        assert!(codes(&module("    let _i = t as i2")).is_empty());
        assert!(codes(&module("    let _t = a as Trit")).contains(&"E2009"));
    }
}

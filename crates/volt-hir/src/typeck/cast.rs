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
        // W2016 (ADR-0116): daraltan dönüşüm işlenenin yalnız alt `k`
        // bitini okur; içindeki en az `k` bitte yapılan kesme görünmez.
        if let (Some((_, _, natural)), Some(k)) =
            (self.types.int_range(src), self.types.width_of(dst))
        {
            if k < natural {
                self.read_low_bits(inner, k);
            }
        }
        self.check_cast_legal(src, dst, span);
        if matches!(self.types.ty(src), Ty::IntLit) {
            self.warn_literal_cut(inner, dst, span);
        }
        dst
    }

    /// ADR-0115 (#207): soneksiz literal dönüşümün hedefine sığmıyorsa
    /// hedefin alt bitleri kalır; kesme tipli daraltma gibi W2010 ile
    /// bildirilir, sessiz değildir. Sığma: `uN`/`bits<N>` için
    /// 0 ≤ K < 2^N, `iN` için -2^(N-1) ≤ K < 2^(N-1).
    fn warn_literal_cut(&mut self, inner: Idx<Expr>, dst: TypeId, span: Span) {
        let (width, signed) = if let Ty::UInt { width } | Ty::Bits { width } = *self.types.ty(dst) {
            (u32::from(width), false)
        } else if let Ty::SInt { width } = *self.types.ty(dst) {
            (u32::from(width), true)
        } else {
            return;
        };
        let Some((neg, mag)) = self.literal_sign_mag(inner) else {
            return;
        };
        let fits = match (signed, width) {
            (_, 0) => false,
            (true, w) if w > 128 => true,
            (true, w) => {
                let half = 1u128 << (w - 1);
                if neg {
                    mag <= half
                } else {
                    mag < half
                }
            }
            (false, w) => !neg && (w >= 128 || mag >> w == 0),
        };
        if fits {
            return;
        }
        let mask = if width >= 128 {
            u128::MAX
        } else {
            (1u128 << width) - 1
        };
        let cut = if neg { mag.wrapping_neg() } else { mag } & mask;
        let value = if neg {
            format!("-{mag}")
        } else {
            mag.to_string()
        };
        let shown = self.show(dst);
        self.warning(
            ErrorCode::W2010,
            span,
            lstr!(en: "the literal {value} does not fit in {shown}; the cast keeps its low {width} bits (0x{cut:X})"; tr: "{value} literali {shown} tipine sığmıyor; dönüşüm alt {width} bitini tutar (0x{cut:X})"),
            lstr!(en: "upper bits are cut"; tr: "üst bitler kesilir"),
            lstr!(en: "write the value that fits (0x{cut:X}) if the cut is intended, or use a wider target"; tr: "kesme bilinçliyse sığan değeri (0x{cut:X}) yazın ya da daha geniş bir hedef kullanın"),
        );
    }

    fn check_cast_legal(&mut self, src: TypeId, dst: TypeId, span: Span) {
        // Esnek aritmetik sonucu doğal genişliğiyle dönüştürülür.
        let src = self.types.concrete(src);
        if src == dst || self.types.is_error(src) || self.types.is_error(dst) {
            return;
        }
        if self.check_struct_cast(src, dst, span) || self.check_enum_cast(src, dst, span) {
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
    use super::super::testutil::{codes, def_ty, diagnostics};

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
    /// #207, ADR-0115: soneksiz literal hedefine sığmıyorsa dönüşüm onu
    /// keser ve W2010 verir (tipli daraltma gibi, §3.6); sığan literal
    /// sessizdir. Sığma: `uN`/`bits<N>` için 0 ≤ K < 2^N, `iN` için
    /// -2^(N-1) ≤ K < 2^(N-1).
    #[test]
    fn issue_207_a_literal_that_does_not_fit_its_cast_target_warns() {
        for cut in [
            "0x1FF as bits<8>",
            "300 as u8",
            "(-1) as u8",
            "(-129) as i8",
            "128 as i8",
            "(200 + 100) as u8",
            "0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF as bits<64>",
            "0x8000_0000_0000_0000_0000_0000_0000_0000 as bits<127>",
        ] {
            let c = codes(&module(&format!("    let _c = {cut}")));
            assert_eq!(c, ["W2010"], "{cut}: {c:?}");
        }
        for fits in [
            "0xFF as bits<8>",
            "255 as u8",
            "(-128) as i8",
            "127 as i8",
            "0 as bits<1>",
            "0x8000_0000_0000_0000_0000_0000_0000_0000 as bits<128>",
            "0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF as bits<256>",
        ] {
            let c = codes(&module(&format!("    let _c = {fits}")));
            assert!(c.is_empty(), "{fits}: {c:?}");
        }
    }

    /// ADR-0115'in iletisi: literal ve kesilmiş değer (hedefin alt bitleri,
    /// ikiye tümleyen). Bileşik literal ifadesinin işareti değerinden gelir:
    /// `(1 - 2) as u8` -1'dir, sığmaz; `(2 - 1) as u8` sığar. 128 bitlik
    /// işaretli hedefte sınır 2^127'dir. Hedefli mutasyon koşusu
    /// 43196528987'nin kaçanları (maske, işaret, `w > 128`); 128 bitten
    /// geniş işaretli hedefe her literal sığar, 128 bitlik işaretsiz hedefte
    /// negatif literal kesilir.
    #[test]
    fn issue_207_literal_cut_message_names_the_value_and_the_cut() {
        for (expr, value, cut) in [
            ("0x1FF as bits<8>", "511", "0xFF"),
            ("(200 + 100) as u8", "300", "0x2C"),
            ("(1 - 2) as u8", "-1", "0xFF"),
            ("(-129) as i8", "-129", "0x7F"),
            (
                "0x3_0000_0000_0000_0000 as u64",
                "55340232221128654848",
                "0x0",
            ),
            (
                "(-1) as bits<128>",
                "-1",
                "0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            ),
        ] {
            let d = diagnostics(&module(&format!("    let _c = {expr}")));
            let w: Vec<_> = d.iter().filter(|d| d.code.as_str() == "W2010").collect();
            assert_eq!(w.len(), 1, "{expr}: {d:?}");
            let want = format!("the literal {value} does not fit");
            assert!(w[0].message.contains(&want), "{expr}: {}", w[0].message);
            assert!(
                w[0].message.contains(&format!("({cut})")),
                "{expr}: {}",
                w[0].message
            );
        }
        for fits in [
            "(2 - 1) as u8",
            "(1 - 1) as u8",
            "0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF as sint<200>",
            "(-0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF) as sint<200>",
            "0x7FFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF as sint<128>",
            "(-0x8000_0000_0000_0000_0000_0000_0000_0000) as sint<128>",
        ] {
            let c = codes(&module(&format!("    let _c = {fits}")));
            assert!(c.is_empty(), "{fits}: {c:?}");
        }
        let c = codes(&module(
            "    let _c = 0x8000_0000_0000_0000_0000_0000_0000_0000 as sint<128>",
        ));
        assert_eq!(c, ["W2010"], "{c:?}");
    }
}

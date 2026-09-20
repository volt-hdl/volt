//! Genişlik ve tam sayı aralığı yardımcıları (type-inference.md §1,
//! §3.3, §4; ADR-0025 esnek genişlik).
//!
//! İki tam sayı tipinin "buluşması" (aynı işaret + kesişen genişlik
//! aralığı) aritmetik, bit düzeyi, karşılaştırma ve koşullu ifade
//! kurallarının ortak sorusudur; tek yerde hesaplanır.

use volt_span::Span;

use super::TypeChecker;
use crate::consteval::MAX_WIDTH;
use crate::ty::{Ty, TypeId};

/// İki tipin tam sayı aralıklarının buluşması.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntMeet {
    /// En az biri tam sayı (uN/iN/esnek) değil.
    NotInts,
    /// İşaretler farklı.
    SignMismatch,
    /// Aynı işaret ama aralıklar kesişmiyor; üst sınırlar tanı içindir.
    Disjoint { signed: bool, lhi: u16, rhi: u16 },
    /// Ortak aralık `lo..=hi`.
    Common { signed: bool, lo: u16, hi: u16 },
}

impl TypeChecker<'_, '_> {
    pub(super) fn meet_int_ranges(&self, lt: TypeId, rt: TypeId) -> IntMeet {
        let (Some((ls, llo, lhi)), Some((rs, rlo, rhi))) =
            (self.types.int_range(lt), self.types.int_range(rt))
        else {
            return IntMeet::NotInts;
        };
        if ls != rs {
            return IntMeet::SignMismatch;
        }
        let lo = llo.max(rlo);
        let hi = lhi.min(rhi);
        if lo > hi {
            return IntMeet::Disjoint {
                signed: ls,
                lhi,
                rhi,
            };
        }
        IntMeet::Common { signed: ls, lo, hi }
    }

    /// İkili operatör operandlarının ortak aralığı. İşaret farkı E2002,
    /// kesişmeyen genişlik E2001 üretir ve `Err(Ty::Error)` döner;
    /// operandlar tam sayı değilse `Ok(None)`.
    pub(super) fn operand_range(
        &mut self,
        lt: TypeId,
        rt: TypeId,
        span: Span,
    ) -> Result<Option<(bool, u16, u16)>, TypeId> {
        match self.meet_int_ranges(lt, rt) {
            IntMeet::NotInts => Ok(None),
            IntMeet::SignMismatch => {
                self.err_sign_mismatch(span);
                Err(self.types.error())
            }
            IntMeet::Disjoint { signed, lhi, rhi } => {
                self.operand_width_mismatch(lhi, rhi, sign_prefix(signed), span);
                Err(self.types.error())
            }
            IntMeet::Common { signed, lo, hi } => Ok(Some((signed, lo, hi))),
        }
    }

    /// `lo..=hi` aralıklı esnek tam sayı tipi.
    pub(super) fn flex(&mut self, signed: bool, lo: u16, hi: u16) -> TypeId {
        if signed {
            self.types.sint_flex(lo, hi)
        } else {
            self.types.uint_flex(lo, hi)
        }
    }

    pub(super) fn is_bits(&self, ty: TypeId) -> bool {
        matches!(self.types.ty(ty), Ty::Bits { .. })
    }

    /// Soneksiz literal bu tipe uyarlanabilir mi? (uN/iN/esnek/Trit)
    pub(super) fn is_literal_adaptable(&self, ty: TypeId) -> bool {
        self.types.int_range(ty).is_some() || matches!(self.types.ty(ty), Ty::Trit)
    }
}

/// Tanı metinlerinde tip ön eki: `i` işaretli, `u` işaretsiz.
pub(super) fn sign_prefix(signed: bool) -> &'static str {
    if signed {
        "i"
    } else {
        "u"
    }
}

/// Genişlemiş sonuç genişliği MAX_WIDTH ve u16 gösterim sınırıyla kırpılır.
pub(super) fn clamp_width(w: u32) -> u16 {
    w.min(MAX_WIDTH).min(u32::from(u16::MAX)) as u16
}

/// `value` işaretsiz `width` bite sığıyor mu?
pub(super) fn uint_fits(value: u128, width: u16) -> bool {
    width >= 128 || value >> width == 0
}

/// `value` işaretli `width` bite (işaret dahil) sığıyor mu?
pub(super) fn sint_fits(value: u128, width: u16) -> bool {
    if width == 0 {
        return false;
    }
    width > 128 || value < 1u128 << (width - 1).min(127)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uint_fits_checks_the_unsigned_maximum() {
        assert!(uint_fits(255, 8));
        assert!(!uint_fits(256, 8));
        assert!(uint_fits(u128::MAX, 128));
    }

    #[test]
    fn sint_fits_reserves_the_sign_bit() {
        assert!(sint_fits(127, 8));
        assert!(!sint_fits(128, 8));
        assert!(!sint_fits(0, 0));
    }

    #[test]
    fn clamp_width_saturates_at_max_width() {
        assert_eq!(clamp_width(9), 9);
        assert_eq!(
            u32::from(clamp_width(u32::MAX)),
            MAX_WIDTH.min(u32::from(u16::MAX))
        );
    }

    #[test]
    fn sign_prefix_matches_type_family() {
        assert_eq!(sign_prefix(true), "i");
        assert_eq!(sign_prefix(false), "u");
    }
}

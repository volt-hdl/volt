//! Bir tipin bit genişliği (#419, ADR-0137 §1: genişlik tek yerden).
//!
//! Genişliği adında yazmayan skaler tiplerin genişliği burada, tek
//! tabloda durur ([`Scalar::bits`]); tip denetimi
//! (`volt_hir::ty::TypeArena::bit_width`) ve AST'yi okuyan arka uçlar
//! ([`type_ref_bits`]: SV testbench'i, test dili) onu okur. #419'dan önce
//! tip denetiminin iki kopyası vardı ve biri `Trit`'e ve `bool`'a genişlik
//! vermiyordu: `ShiftRegister<Trit, N>`'in `taps`'ı `bits<N>` sayılıyor
//! (SV'de 2N bit), `Trit`'in `sync()`'i W3003 almıyordu.
//!
//! Genişliği olmayan tip (demet, çözülemeyen ad, sabit olmayan `bits<N>`)
//! `None` verir; çağıran onu bir varsayılanla (1 bit) doldurmaz.

use crate::{const_eval, enum_layout, struct_layout, Idx, SourceFile, TypeRef, TypeRefKind};

/// Genişliği adında yazmayan skaler tip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scalar {
    Bool,
    Clock,
    Reset,
    Trit,
}

impl Scalar {
    /// Donanımdaki bit genişliği: `bool`, `clock` ve `reset` 1 bit;
    /// `Trit` 2 bit işaretli depolamadır (ADR-0003, ADR-0062: +1 = 01,
    /// 0 = 00, -1 = 11).
    pub const fn bits(self) -> u32 {
        match self {
            Scalar::Bool | Scalar::Clock | Scalar::Reset => 1,
            Scalar::Trit => 2,
        }
    }

    /// Tip başvurusu skaler tiplerden biriyse o.
    pub fn of(kind: &TypeRefKind) -> Option<Scalar> {
        match kind {
            TypeRefKind::Bool => Some(Scalar::Bool),
            TypeRefKind::Clock => Some(Scalar::Clock),
            TypeRefKind::Reset(_) => Some(Scalar::Reset),
            TypeRefKind::Trit => Some(Scalar::Trit),
            TypeRefKind::UInt(_)
            | TypeRefKind::SInt(_)
            | TypeRefKind::Bits(_)
            | TypeRefKind::UIntN(_)
            | TypeRefKind::SIntN(_)
            | TypeRefKind::Array { .. }
            | TypeRefKind::Tuple(_)
            | TypeRefKind::Path { .. }
            | TypeRefKind::Error => None,
        }
    }
}

/// İç içe tip (dizi, takma ad) için özyineleme sınırı: birbirini dizi
/// elemanı olarak adlandıran iki takma ad sonsuz döngü kurmasın.
const MAX_TYPE_DEPTH: u32 = 16;

/// AST tip başvurusunun bit genişliği; tip denetiminin
/// `TypeArena::bit_width`'iyle aynı kural, AST'den: takma ad hedef
/// tipidir (ADR-0070 §6), skaler tip [`Scalar::bits`], `uN`/`iN` N,
/// `bits<N>`/`uint<N>`/`sint<N>` sabit N ([`const_eval::width`]), dizi
/// eleman × uzunluk (ADR-0056), enum kodlama genişliği (ADR-0074), struct
/// düzen genişliği `W` (ADR-0077). Çözülemeyen tip `None`.
pub fn type_ref_bits(ast: &SourceFile, ty: Idx<TypeRef>) -> Option<u32> {
    bits_at(ast, ty, 0)
}

fn bits_at(ast: &SourceFile, ty: Idx<TypeRef>, depth: u32) -> Option<u32> {
    if depth > MAX_TYPE_DEPTH {
        return None;
    }
    let ty = crate::alias::alias_chain_end(ast, ty).ok()?;
    let kind = &ast.types[ty].kind;
    if let Some(scalar) = Scalar::of(kind) {
        return Some(scalar.bits());
    }
    let top = const_eval::TopLevel(ast);
    let eval = &mut |e| const_eval::const_int(ast, e);
    match kind {
        TypeRefKind::UInt(n) | TypeRefKind::SInt(n) => Some(u32::from(*n)),
        TypeRefKind::Bits(e) | TypeRefKind::UIntN(e) | TypeRefKind::SIntN(e) => {
            const_eval::width(ast, &top, *e).ok()
        }
        TypeRefKind::Array { elem, len } => {
            let len = u32::try_from(const_eval::const_int(ast, *len)?).ok()?;
            bits_at(ast, *elem, depth + 1)?.checked_mul(len)
        }
        TypeRefKind::Path { .. } => {
            if let Some(decl) = enum_layout::enum_of_type(ast, ty) {
                return enum_layout::valid_layout(ast, decl, eval).map(|l| l.width);
            }
            let decl = struct_layout::struct_of_type(ast, ty)?;
            struct_layout::layout(ast, decl, eval).ok().map(|l| l.width)
        }
        TypeRefKind::Bool
        | TypeRefKind::Clock
        | TypeRefKind::Reset(_)
        | TypeRefKind::Trit
        | TypeRefKind::Tuple(_)
        | TypeRefKind::Error => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_widths_are_one_bit_except_trit() {
        assert_eq!(Scalar::Bool.bits(), 1);
        assert_eq!(Scalar::Clock.bits(), 1);
        assert_eq!(Scalar::Reset.bits(), 1);
        assert_eq!(Scalar::Trit.bits(), 2);
    }
}

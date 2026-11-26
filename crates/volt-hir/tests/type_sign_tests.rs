//! Değerin işareti tek fonksiyondan gelir: `TypeArena::is_signed` (#413,
//! ADR-0137 §1, ADR-0141). `prev()`'in yardımcı register'ı onu okur.
//! Kural elle, spec'ten: `iN` ve işaretli esnek aralık işaretli
//! (type-inference.md §1, ADR-0025); `Trit` 2 bit işaretli depolama
//! (ADR-0003, ADR-0062); `Delayed<T, N>` `T`'nin tipidir (ADR-0037);
//! `uN`, `bits`, `bool`, enum (kodlama, ADR-0074), struct ve dizi
//! (paketlenmiş vektör, ADR-0056, ADR-0077) işaretsiz.

use volt_hir::ty::{EnumId, StructId, Ty, TypeArena};

#[test]
fn signed_types_are_sint_signed_ranges_and_trit() {
    let mut t = TypeArena::new();
    let i8 = t.intern(Ty::SInt { width: 8 });
    let u8 = t.intern(Ty::UInt { width: 8 });
    for (ty, want) in [
        (i8, true),
        (t.intern(Ty::SIntFlex { lo: 8, hi: 9 }), true),
        (t.intern(Ty::Trit), true),
        (u8, false),
        (t.intern(Ty::UIntFlex { lo: 8, hi: 9 }), false),
        (t.intern(Ty::Bits { width: 8 }), false),
        (t.intern(Ty::Bool), false),
        (t.intern(Ty::Enum(EnumId(1))), false),
        (t.intern(Ty::Struct(StructId(2))), false),
        (t.intern(Ty::Array { elem: i8, len: 4 }), false),
        (t.intern(Ty::IntLit), false),
        (t.intern(Ty::Error), false),
    ] {
        assert_eq!(t.is_signed(ty), want, "{}", t.display(ty));
    }
}

#[test]
fn a_delayed_value_has_the_sign_of_its_type() {
    let mut t = TypeArena::new();
    let i8 = t.intern(Ty::SInt { width: 8 });
    let u8 = t.intern(Ty::UInt { width: 8 });
    let late_signed = t.intern(Ty::Delayed {
        inner: i8,
        cycles: 2,
    });
    let late_unsigned = t.intern(Ty::Delayed {
        inner: u8,
        cycles: 2,
    });
    assert!(t.is_signed(late_signed));
    assert!(!t.is_signed(late_unsigned));
}

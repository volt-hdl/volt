//! Yerleşik primitiflerin veri tipi `T` (ADR-0087).
//!
//! Struct `T` bellekte / register'da ADR-0077 Karar 3 düzeniyle TEK
//! paketlenmiş vektördür (ilk alan MSB): `SyncFifo<Packet, 8>` bir
//! `logic [11:0] mem [8]` — alan başına bellek (SoA) değil, BRAM
//! çıkarımı korunur. Paketleme/açma `structs` indirgemesindedir (`as uW`,
//! `as P`); burada genişlik ve reset değeri.
//!
//! Reset değeri `T`'nin VARSAYILAN kodlamasıdır: enum yaprağı ilk
//! varyantının kodu, diğer yapraklar sıfır. FIFO ailesinin çıkış
//! register'ı yalnız yazılmış (geçerli) değerleri ya da bu değeri
//! gösterir — enum yaprağına hiç geçersiz kod düşmez. Varsayılan sıfırsa
//! (ilk varyant kodu 0 — ADR-0074'te açık değer yoksa hep) çıktı eskisiyle
//! aynıdır.

use volt_ast::{enum_layout, struct_layout, Idx, SourceFile, TypeRef};

/// Struct `T`'nin paketlenmiş genişliği; `T` struct değilse `None`.
/// Düzen hatası (HIR tanısı verdi) `Some(None)`.
pub(crate) fn struct_width(ast: &SourceFile, ty: Idx<TypeRef>) -> Option<Option<u32>> {
    let decl = struct_layout::struct_of_type(ast, ty)?;
    Some(
        struct_layout::layout(ast, decl, &mut |e| crate::structs::const_int(ast, e))
            .ok()
            .map(|l| l.width),
    )
}

/// `T`'nin varsayılan kodlaması `width` bitlik SV literali olarak; bütün
/// bitler sıfırsa `None` (çağıran `zero_of` kullanır — çıktı değişmez).
pub(crate) fn default_literal(ast: &SourceFile, ty: Idx<TypeRef>, width: u32) -> Option<String> {
    let mut bits = vec![false; width as usize];
    if let Some(decl) = struct_layout::struct_of_type(ast, ty) {
        let layout =
            struct_layout::layout(ast, decl, &mut |e| crate::structs::const_int(ast, e)).ok()?;
        for leaf in layout
            .leaves
            .iter()
            .filter(|l| l.kind == struct_layout::LeafKind::Enum)
        {
            let code = first_code(ast, leaf.ty)?;
            set(&mut bits, leaf.lsb, leaf.width, code);
        }
    } else {
        set(&mut bits, 0, width, first_code(ast, ty)?);
    }
    bits.iter().any(|&b| b).then(|| literal(&bits))
}

/// Enum tipinin ilk varyantının kodu (enum değilse `None`).
fn first_code(ast: &SourceFile, ty: Idx<TypeRef>) -> Option<u128> {
    let decl = enum_layout::enum_of_type(ast, ty)?;
    let layout = enum_layout::valid_layout(ast, decl, &mut |e| crate::structs::const_int(ast, e))?;
    layout.values.first().copied()
}

fn set(bits: &mut [bool], lsb: u32, width: u32, value: u128) {
    for i in 0..width.min(128) {
        if let Some(b) = bits.get_mut((lsb + i) as usize) {
            *b = (value >> i) & 1 == 1;
        }
    }
}

/// `W'hXX..` (MSB solda).
fn literal(bits: &[bool]) -> String {
    let w = bits.len();
    let digits = w.div_ceil(4);
    let mut hex = String::with_capacity(digits);
    for d in (0..digits).rev() {
        let mut nibble = 0u8;
        for i in 0..4 {
            if bits.get(d * 4 + i).copied().unwrap_or(false) {
                nibble |= 1 << i;
            }
        }
        hex.push(char::from_digit(u32::from(nibble), 16).unwrap_or('0'));
    }
    format!("{w}'h{}", hex.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::literal;

    #[test]
    fn literal_is_msb_first_hex() {
        // 12 bit: 0x0A5 → bit 0, 2, 5, 7.
        let mut bits = vec![false; 12];
        for i in [0, 2, 5, 7] {
            bits[i] = true;
        }
        assert_eq!(literal(&bits), "12'h0A5");
        assert_eq!(literal(&[true, false, true]), "3'h5");
    }
}

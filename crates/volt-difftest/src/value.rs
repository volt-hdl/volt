//! Bit örüntüsü yardımcıları. Değerler `u128` örüntüsüdür; genişlik ve
//! işaret tipten gelir. Modelin bütün genişlikleri 128'in altındadır
//! (tam sayılar en çok 64, `bits<N>` en çok 70 bit).

pub fn mask(w: u32) -> u128 {
    if w >= 128 {
        u128::MAX
    } else {
        (1u128 << w) - 1
    }
}

/// `from_w` bitlik `v`'yi `to_w` bite dönüştürür: dar hedefe kesilir,
/// geniş hedefe işaretliyse işaretle, değilse sıfırla genişletilir.
pub fn conv(v: u128, from_w: u32, signed: bool, to_w: u32) -> u128 {
    let v = v & mask(from_w);
    if to_w <= from_w {
        return v & mask(to_w);
    }
    if signed && from_w > 0 && (v >> (from_w - 1)) & 1 == 1 {
        (v | !mask(from_w)) & mask(to_w)
    } else {
        v
    }
}

/// Örüntünün sayı değeri.
pub fn to_i128(v: u128, w: u32, signed: bool) -> i128 {
    let v = v & mask(w);
    if signed && w > 0 && (v >> (w - 1)) & 1 == 1 {
        (v | !mask(w)) as i128
    } else {
        v as i128
    }
}

/// `v` `w` bitlik işaretli/işaretsiz aralığa sığar mı?
pub fn fits(v: i128, w: u32, signed: bool) -> bool {
    if signed {
        let half = 1i128 << (w - 1);
        (-half..half).contains(&v)
    } else {
        v >= 0 && (v as u128) <= mask(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conv_extends_by_sign_and_truncates() {
        assert_eq!(conv(0xFF, 8, true, 12), 0xFFF);
        assert_eq!(conv(0xFF, 8, false, 12), 0x0FF);
        assert_eq!(conv(0x1FF, 9, false, 8), 0xFF);
        assert_eq!(to_i128(0x80, 8, true), -128);
        assert_eq!(to_i128(0x80, 8, false), 128);
        assert!(fits(-128, 8, true) && !fits(128, 8, true));
        assert!(fits(255, 8, false) && !fits(256, 8, false) && !fits(-1, 8, false));
    }
}

//! İngilizce ileti dilbilgisi yardımcıları.

/// Sayıdan önceki İngilizce belirsiz artikel: sayı okunuşu ünlüyle
/// başlıyorsa "an" (eight, eleven, eighteen, eighty, eight hundred,
/// eleven thousand...), değilse "a" ("a 16-bit value", "an 8-bit value").
///
/// Okunuş üçlü gruplarla yapılır: en soldaki grup 8 ile başlıyorsa
/// (8, 80–89, 800–899) ya da tam 11 veya 18 ise "an". 1100 "one thousand
/// one hundred" diye okunur ("a"); 11000 "eleven thousand" ("an").
pub fn a_an(n: u64) -> &'static str {
    let mut lead = n;
    while lead >= 1000 {
        lead /= 1000;
    }
    let first_digit = {
        let mut d = lead;
        while d >= 10 {
            d /= 10;
        }
        d
    };
    if first_digit == 8 || lead == 11 || lead == 18 {
        "an"
    } else {
        "a"
    }
}

#[cfg(test)]
mod tests {
    use super::a_an;

    #[test]
    fn an_before_numbers_read_with_a_vowel_sound() {
        for n in [
            8, 11, 18, 80, 81, 85, 89, 800, 899, 8000, 11_000, 18_000, 80_000,
        ] {
            assert_eq!(a_an(n), "an", "{n}");
        }
    }

    #[test]
    fn a_before_other_numbers() {
        for n in [
            0, 1, 2, 4, 7, 9, 10, 12, 16, 17, 19, 32, 64, 79, 90, 100, 110, 111, 118, 180, 1000,
            1100, 1800, 16_000, 65_535,
        ] {
            assert_eq!(a_an(n), "a", "{n}");
        }
    }
}

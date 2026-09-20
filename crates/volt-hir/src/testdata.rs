//! Test veri dosyaları (ADR-0058): yol kuralı, `$readmemh` ayrıştırma
//! ve dosya erişim arayüzü.
//!
//! Bu crate dosya sistemi okumaz: gerçek okuma sürücünün verdiği
//! `TestFileLoader` üzerinden yapılır. Yükleyici yokken (tek dosyalık
//! `analyze`, LSP) yalnız sözcüksel yol kuralı denetlenir ve dosya
//! içeriği "bilinmiyor" sayılır.

/// `read_hex` dizisinin kabul ettiği en büyük eleman sayısı: en büyük
/// dizi yazmacı kadar (`MAX_ARRAY_LEN`, 1M) — daha uzunu hiçbir `load`
/// hedefine sığmaz, `@FFFFFF` gibi bir adres de belleği tüketmesin.
pub const MAX_HEX_ELEMENTS: usize = crate::consteval::MAX_ARRAY_LEN;

/// Dosya erişim hatası — ikisi de E8507 üretir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestFileError {
    /// Yol proje kökünün dışına çıkıyor.
    OutsideProject,
    /// Dosya yok ya da okunamadı (işletim sistemi iletisi).
    NotFound(String),
}

/// Test dosyasına göre göreli bir veri dosyasını okur.
pub trait TestFileLoader {
    fn load(&self, rel_path: &str) -> Result<String, TestFileError>;
}

/// Sözcüksel yol kuralı: mutlak yollar ve kökü `..` ile aşan yollar
/// reddedilir. `depth_allowance`, test dosyasının proje kökünün kaç
/// dizin altında olduğudur (kök bilinmiyorsa 0 — test dosyasının
/// dizini kök sayılır). Dönen değer normalleştirilmiş parçalardır
/// (baştaki `..`'lar korunur).
pub fn normalize_data_path(rel_path: &str, depth_allowance: usize) -> Option<Vec<String>> {
    let unified = rel_path.replace('\\', "/");
    let bytes = unified.as_bytes();
    let has_drive = bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic();
    if unified.is_empty() || unified.starts_with('/') || has_drive {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    let mut ups = 0usize;
    for seg in unified.split('/') {
        if !is_plain_segment(seg) {
            return None;
        }
        match seg {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    ups += 1;
                }
            }
            other => parts.push(other.to_string()),
        }
    }
    if ups > depth_allowance || parts.is_empty() {
        return None;
    }
    let mut out = vec!["..".to_string(); ups];
    out.extend(parts);
    Some(out)
}

/// Yol parçası işletim sistemine göre anlam değiştirmemeli: `:` Windows'ta
/// sürücü-göreli yol (`C:x` — `join` taban dizini atar) ve alternatif veri
/// akışıdır; kontrol karakterleri ile sondaki nokta/boşluk da sessizce
/// başka bir dosyaya çözülür.
fn is_plain_segment(seg: &str) -> bool {
    if seg.is_empty() || seg == "." || seg == ".." {
        return true;
    }
    !seg.contains(|c: char| c == ':' || c.is_control()) && !seg.ends_with(['.', ' '])
}

/// Ayrıştırılmış `$readmemh` görüntüsü.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexImage {
    /// 0. elemandan en yüksek yazılan elemana kadar; boşluklar 0.
    pub words: Vec<u64>,
    /// En geniş kelimenin basamak sayısından çıkarılan eleman genişliği
    /// (8, 16, 32 ya da 64 bit).
    pub elem_bits: u8,
}

/// `$readmemh` biçim hatası: 1-tabanlı satır + İngilizce kısa neden
/// (tanı iletisine gömülür).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexError {
    pub line: usize,
    pub reason: HexErrorReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HexErrorReason {
    BadDigit(char),
    UnknownDigit,
    TooWide,
    /// `@` sonrası adres yok.
    MissingAddress,
    AddressTooLarge,
    UnclosedComment,
    Empty,
}

/// Yorumları boşlukla değiştirir (satır sayısı korunur).
fn strip_comments(text: &str) -> Result<String, HexError> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut line = 1usize;
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('/', Some('/')) => {
                while chars.peek().is_some_and(|n| *n != '\n') {
                    chars.next();
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let opened_at = line;
                let mut closed = false;
                while let Some(n) = chars.next() {
                    if n == '\n' {
                        line += 1;
                        out.push('\n');
                    } else if n == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return Err(HexError {
                        line: opened_at,
                        reason: HexErrorReason::UnclosedComment,
                    });
                }
                out.push(' ');
            }
            _ => {
                if c == '\n' {
                    line += 1;
                }
                out.push(c);
            }
        }
    }
    Ok(out)
}

/// Tek belirteci (kelime ya da `@` sonrası adres) değere çevirir;
/// dönen ikinci değer anlamlı basamak sayısıdır.
fn parse_hex_token(token: &str, line: usize) -> Result<(u64, usize), HexError> {
    let mut value = 0u64;
    let mut digits = 0usize;
    for c in token.chars().filter(|c| *c != '_') {
        let Some(d) = c.to_digit(16) else {
            let reason = if matches!(c, 'x' | 'X' | 'z' | 'Z' | '?') {
                HexErrorReason::UnknownDigit
            } else {
                HexErrorReason::BadDigit(c)
            };
            return Err(HexError { line, reason });
        };
        digits += 1;
        if digits > 16 {
            return Err(HexError {
                line,
                reason: HexErrorReason::TooWide,
            });
        }
        value = (value << 4) | u64::from(d);
    }
    if digits == 0 {
        return Err(HexError {
            line,
            reason: HexErrorReason::BadDigit('_'),
        });
    }
    Ok((value, digits))
}

/// Verilog `$readmemh` metnini ayrıştırır: boşlukla ayrılmış onaltılık
/// kelimeler, `_`, `//` ve `/* */` yorumları, `@adres` (eleman cinsinden).
pub fn parse_readmemh(text: &str) -> Result<HexImage, HexError> {
    // Windows editörlerinin eklediği UTF-8 BOM veri değildir.
    let clean = strip_comments(text.strip_prefix('\u{feff}').unwrap_or(text))?;
    let mut words: Vec<u64> = Vec::new();
    let mut cursor = 0usize;
    let mut max_digits = 0usize;
    for (i, line_text) in clean.lines().enumerate() {
        let line = i + 1;
        for token in line_text.split_whitespace() {
            if let Some(addr) = token.strip_prefix('@') {
                if addr.is_empty() {
                    return Err(HexError {
                        line,
                        reason: HexErrorReason::MissingAddress,
                    });
                }
                let (value, _) = parse_hex_token(addr, line)?;
                cursor = usize::try_from(value).unwrap_or(usize::MAX);
                continue;
            }
            let (value, digits) = parse_hex_token(token, line)?;
            if cursor >= MAX_HEX_ELEMENTS {
                return Err(HexError {
                    line,
                    reason: HexErrorReason::AddressTooLarge,
                });
            }
            if cursor >= words.len() {
                words.resize(cursor + 1, 0);
            }
            words[cursor] = value;
            cursor += 1;
            max_digits = max_digits.max(digits);
        }
    }
    if words.is_empty() {
        return Err(HexError {
            line: 1,
            reason: HexErrorReason::Empty,
        });
    }
    let elem_bits = match max_digits {
        0..=2 => 8,
        3..=4 => 16,
        5..=8 => 32,
        _ => 64,
    };
    Ok(HexImage { words, elem_bits })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_words_infer_u32() {
        let img = parse_readmemh("00000013 00000093\nDEADBEEF\n").expect("geçerli");
        assert_eq!(img.words, vec![0x13, 0x93, 0xDEAD_BEEF]);
        assert_eq!(img.elem_bits, 32);
    }

    #[test]
    fn byte_dump_infers_u8() {
        let img = parse_readmemh("@00000000\n97 02 00 00\n").expect("geçerli");
        assert_eq!(img.words, vec![0x97, 0x02, 0, 0]);
        assert_eq!(img.elem_bits, 8);
    }

    #[test]
    fn address_jump_zero_fills_gap() {
        let img = parse_readmemh("1 2\n@4\nA\n").expect("geçerli");
        assert_eq!(img.words, vec![1, 2, 0, 0, 0xA]);
    }

    #[test]
    fn address_can_move_backwards_and_overwrite() {
        let img = parse_readmemh("1 2 3\n@1 FF\n").expect("geçerli");
        assert_eq!(img.words, vec![1, 0xFF, 3]);
    }

    #[test]
    fn comments_and_underscores_are_ignored() {
        let src = "// başlık\n0000_0013 /* araya\n giren */ 0000_0093\n";
        let img = parse_readmemh(src).expect("geçerli");
        assert_eq!(img.words, vec![0x13, 0x93]);
    }

    #[test]
    fn widths_follow_the_widest_token() {
        assert_eq!(parse_readmemh("1FF").expect("ok").elem_bits, 16);
        assert_eq!(parse_readmemh("1_0000_0000").expect("ok").elem_bits, 64);
    }

    #[test]
    fn bad_digit_reports_line() {
        let err = parse_readmemh("00\n0G\n").expect_err("hata");
        assert_eq!(err.line, 2);
        assert_eq!(err.reason, HexErrorReason::BadDigit('G'));
    }

    #[test]
    fn x_and_z_digits_are_rejected() {
        let err = parse_readmemh("0x\n").expect_err("hata");
        assert_eq!(err.reason, HexErrorReason::UnknownDigit);
    }

    #[test]
    fn more_than_64_bits_is_rejected() {
        let err = parse_readmemh("11112222333344445\n").expect_err("hata");
        assert_eq!(err.reason, HexErrorReason::TooWide);
    }

    #[test]
    fn address_without_digits_and_bom_are_handled() {
        let err = parse_readmemh("@ 4\n1\n").expect_err("hata");
        assert_eq!(err.reason, HexErrorReason::MissingAddress);
        let img = parse_readmemh("\u{feff}0A 0B\n").expect("BOM soyulur");
        assert_eq!(img.words, vec![0x0A, 0x0B]);
    }

    #[test]
    fn address_limit_matches_the_largest_register_array() {
        assert!(parse_readmemh("@FFFFF 1\n").is_ok());
        assert_eq!(
            parse_readmemh("@100000 1\n").expect_err("hata").reason,
            HexErrorReason::AddressTooLarge
        );
    }

    #[test]
    fn segments_that_change_meaning_on_windows_are_rejected() {
        // Sürücü-göreli parça `join`de taban dizini atar; ADS ve sondaki
        // nokta/boşluk başka dosyaya çözülür.
        assert_eq!(normalize_data_path("sub/C:ok.hex", 0), None);
        assert_eq!(normalize_data_path("ok.hex::$DATA", 0), None);
        assert_eq!(normalize_data_path("ok.hex.", 0), None);
        assert_eq!(normalize_data_path("sub /ok.hex", 0), None);
        assert_eq!(normalize_data_path("a\u{0}b.hex", 0), None);
    }

    #[test]
    fn huge_address_is_rejected_not_allocated() {
        let err = parse_readmemh("@FFFFFFFF 1\n").expect_err("hata");
        assert_eq!(err.reason, HexErrorReason::AddressTooLarge);
    }

    #[test]
    fn empty_and_unclosed_comment_are_rejected() {
        assert_eq!(
            parse_readmemh("// yalnız yorum\n")
                .expect_err("hata")
                .reason,
            HexErrorReason::Empty
        );
        assert_eq!(
            parse_readmemh("1 /* açık\n").expect_err("hata").reason,
            HexErrorReason::UnclosedComment
        );
    }

    #[test]
    fn path_inside_project_is_normalized() {
        assert_eq!(
            normalize_data_path("sw/./hello.hex", 0),
            Some(vec!["sw".to_string(), "hello.hex".to_string()])
        );
        assert_eq!(
            normalize_data_path("sw\\..\\hello.hex", 0),
            Some(vec!["hello.hex".to_string()])
        );
    }

    #[test]
    fn escaping_and_absolute_paths_are_rejected() {
        assert_eq!(normalize_data_path("../x.hex", 0), None);
        assert_eq!(normalize_data_path("a/../../x.hex", 0), None);
        assert_eq!(normalize_data_path("..\\..\\x.hex", 1), None);
        assert_eq!(normalize_data_path("/etc/passwd", 9), None);
        assert_eq!(normalize_data_path("C:\\Windows\\x.hex", 9), None);
        assert_eq!(normalize_data_path("", 0), None);
        assert_eq!(normalize_data_path(".", 0), None);
    }

    #[test]
    fn parent_is_allowed_up_to_the_project_root() {
        assert_eq!(
            normalize_data_path("../data/x.hex", 1),
            Some(vec![
                "..".to_string(),
                "data".to_string(),
                "x.hex".to_string()
            ])
        );
    }
}

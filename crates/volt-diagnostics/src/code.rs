//! Hata ve uyarı kodları.
//!
//! Kaynaklar: grammar-full.ebnf §18, name-resolution.md, type-inference.md,
//! const-eval.md, domain-inference.md, Volt-Dil-Spesifikasyonu-v3.md.
//! `volt explain <KOD>` bu tablodan beslenir; açıklama metinleri
//! `messages/{en,tr}.rs` içinde yaşar (GLOSSARY.md terminolojisi).

use std::fmt;

use crate::messages;

macro_rules! error_codes {
    ($($variant:ident),* $(,)?) => {
        /// Spec'te tanımlı tüm tanı kodları.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[allow(clippy::upper_case_acronyms)]
        pub enum ErrorCode {
            $($variant),*
        }

        impl ErrorCode {
            /// Tanımlı tüm kodlar (volt explain listelemesi için).
            pub const ALL: &'static [ErrorCode] = &[$(ErrorCode::$variant),*];

            /// "E3001" biçiminde kod metni.
            pub fn as_str(&self) -> &'static str {
                match self { $(ErrorCode::$variant => stringify!($variant)),* }
            }
        }
    };
}

error_codes! {
    // ─── Sözdizimi (grammar-full.ebnf §18) ───
    E0001, E0002, E0003, E0004, E0005, E0006, E0007, E0008, E0009, E0010,
    E0011, E0012, E0013,

    // ─── İsim çözümleme (name-resolution.md) ───
    E1001, E1002, E1003, E1004, E1005, E1006, E1007, E1008, E1009, E1010,

    // ─── Tip çıkarımı (type-inference.md) ───
    E2001, E2002, E2003, E2004, E2005, E2006, E2007, E2008, E2009, E2010,
    E2011, E2012,

    // ─── Sabit değerlendirme (const-eval.md) ───
    E2020, E2021, E2022, E2023, E2024, E2025, E2026, E2027, E2028, E2029,

    // ─── Saat/sıfırlama alanları (domain-inference.md) ───
    E3001, E3002, E3003, E3004, E3005, E3006, E3007, E3008, E3009, E3010,
    E3011, E3012,

    // ─── Bağlantı/sürücü (type-inference.md) ───
    E4001, E4002, E4003, E4004,

    // ─── Davranışsal kontratlar (contracts) ───
    E5001, E5004,

    // ─── Bütçe ve zamanlama kontratları ───
    E6001, E6003, E6004,

    // ─── Sürümleme ───
    E7001, E7002,

    // ─── Release disiplini ───
    E9001, E9002,

    // ─── Uyarılar ───
    W0010, W0020, W0021, W1001, W1002, W1003, W1004, W1005, W2010, W2011,
    W2012, W2013, W2020, W2021, W3001, W3002, W3003, W3004, W3005, W4001,
    W4002,
}

impl ErrorCode {
    /// Aktif dildeki kısa açıklama (messages/{en,tr}.rs).
    pub fn description(&self) -> &'static str {
        messages::message(messages::lang(), *self)
    }

    /// "E3001" biçimindeki metni koda çevirir (büyük/küçük harf
    /// duyarsız). Tanınmayan kod için `None` — `volt explain` çıkış
    /// kodu 2 ile öneri üretir.
    pub fn parse(s: &str) -> Option<ErrorCode> {
        let upper = s.trim().to_ascii_uppercase();
        ErrorCode::ALL.iter().copied().find(|c| c.as_str() == upper)
    }

    /// W ile başlayan kodlar uyarıdır.
    pub fn is_warning(&self) -> bool {
        self.as_str().starts_with('W')
    }

    /// `volt explain` sayfası (cli-contract.md §5 JSON `explain_url`).
    pub fn explain_url(&self) -> String {
        format!("https://volthdl.org/errors/{}", self.as_str())
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.as_str(), self.description())
    }
}

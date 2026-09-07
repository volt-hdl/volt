//! Tanı mesajı yerelleştirmesi (GLOSSARY.md §7 şablonu).
//!
//! Dil seçimi driver tarafından süreç başında yapılır:
//! --lang bayrağı > VOLT_LANG > Volt.toml [ui] lang > En (varsayılan).
//! Sistem locale'i BİLEREK okunmaz — CI'da sürpriz üretir.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::code::ErrorCode;

pub mod en;
pub mod tr;

/// Desteklenen çıktı dilleri. `Default` = İngilizce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Tr,
}

impl Lang {
    /// "en" | "tr" (büyük/küçük harf duyarsız). Tanınmayan değer `None`.
    pub fn parse(s: &str) -> Option<Lang> {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" => Some(Lang::En),
            "tr" => Some(Lang::Tr),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Tr => "tr",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0); // 0 = En, 1 = Tr

/// Süreç genelinde aktif dili ayarlar (driver başlangıçta çağırır).
pub fn set_lang(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

/// Aktif dil.
pub fn lang() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::Tr,
        _ => Lang::En,
    }
}

/// Kod açıklaması — dil dağıtıcısı (dispatcher).
pub fn message(lang: Lang, code: ErrorCode) -> &'static str {
    match lang {
        Lang::En => en::description(code),
        Lang::Tr => tr::description(code),
    }
}

/// İnsan çıktısındaki "= anahtar:" satır başlıkları (GLOSSARY.md §7).
pub struct TemplateKeys {
    pub reason: &'static str,
    pub note: &'static str,
    pub counterexample: &'static str,
    pub help: &'static str,
    pub for_more: &'static str,
}

pub fn keys(lang: Lang) -> TemplateKeys {
    match lang {
        Lang::En => TemplateKeys {
            reason: "reason:",
            note: "note:",
            counterexample: "counterexample:",
            help: "help:",
            for_more: "for more:",
        },
        Lang::Tr => TemplateKeys {
            reason: "neden:",
            note: "not:",
            counterexample: "karşı örnek:",
            help: "çözüm:",
            for_more: "daha fazla:",
        },
    }
}

/// Aktif dile göre biçimlenmiş metin: `lstr!(en: "...", args; tr: "...", args)`.
///
/// Kod parçacıkları, sinyal/modül isimleri, `sync()` gibi API adları ve
/// dosya yolları her iki kolda da ÇEVRİLMEDEN kalır (GLOSSARY.md §0).
#[macro_export]
macro_rules! lstr {
    (en: $efmt:literal $(, $earg:expr)* ; tr: $tfmt:literal $(, $targ:expr)* $(;)?) => {
        match $crate::messages::lang() {
            $crate::messages::Lang::Tr => format!($tfmt $(, $targ)*),
            $crate::messages::Lang::En => format!($efmt $(, $earg)*),
        }
    };
}

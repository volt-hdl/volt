//! Hata ve uyarı kodları.
//!
//! Kaynaklar: grammar-full.ebnf §18, name-resolution.md, type-inference.md,
//! const-eval.md, domain-inference.md, Volt-Dil-Spesifikasyonu-v3.md.
//! `volt explain <KOD>` bu tablodan beslenir.

use std::fmt;

macro_rules! error_codes {
    ($($variant:ident => $desc:expr),* $(,)?) => {
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

            /// Kısa Türkçe açıklama.
            pub fn description(&self) -> &'static str {
                match self { $(ErrorCode::$variant => $desc),* }
            }
        }
    };
}

error_codes! {
    // ─── Sözdizimi (grammar-full.ebnf §18) ───
    E0001 => "Beklenmeyen token",
    E0002 => "Eksik kapanış: } ) ]",
    E0003 => "Anahtar kelime ayrılmış, henüz desteklenmiyor",
    E0004 => "Blok sonlandırma ismi uyuşmuyor",
    E0005 => "Geçersiz sayısal literal",
    E0006 => "Sıralı blokta '=' kullanıldı ('<=' olmalı)",
    E0007 => "Kombinasyonel blokta '<=' kullanıldı ('=' olmalı)",
    E0008 => "'if' ifadesinde 'else' eksik (latch riski)",
    E0009 => "Geçersiz nitelik (attribute) argümanı",
    E0010 => "Karşılaştırma operatörleri zincirlenemez",
    E0011 => "Beklenmeyen dosya sonu",
    E0012 => "Geçersiz escape dizisi",
    E0013 => "Kapanmamış blok yorumu",

    // ─── İsim çözümleme (name-resolution.md) ───
    E1001 => "Tanımsız isim",
    E1002 => "Bildirimden önce kullanım",
    E1003 => "Aynı kapsamda çift tanım",
    E1004 => "Özel (pub olmayan) öğeye erişim",
    E1005 => "Ad alanı olmayan öğede '::' kullanımı",
    E1006 => "Döngüsel modül bağımlılığı",
    E1007 => "Enum varyantı bulunamadı",
    E1008 => "Struct alanı bulunamadı",
    E1009 => "Modül portu bulunamadı",
    E1010 => "Belirsiz import (iki 'use' aynı ismi getiriyor)",

    // ─── Tip çıkarımı (type-inference.md) ───
    E2001 => "Bit genişliği uyumsuzluğu",
    E2002 => "İşaret uyumsuzluğu",
    E2003 => "Tip uyumsuzluğu (genel)",
    E2004 => "bits<N> tipinde aritmetik",
    E2005 => "Literal genişliği belirlenemiyor",
    E2006 => "İndeks/aralık sınır dışı",
    E2007 => "Ters aralık (hi < lo)",
    E2008 => "Değişken aralık sınırı",
    E2009 => "Geçersiz tip dönüşümü",
    E2010 => "Literal hedef tipe sığmıyor",
    E2011 => "Geçersiz Trit literali",
    E2012 => "Register tipi belirlenemiyor",

    // ─── Sabit değerlendirme (const-eval.md) ───
    E2020 => "Döngüsel sabit bağımlılığı",
    E2021 => "Sabit ifade bekleniyor (çalışma zamanı değeri kullanıldı)",
    E2022 => "Derleme zamanı taşması",
    E2023 => "Sıfıra bölme",
    E2024 => "Geçersiz kaydırma miktarı",
    E2025 => "Geçersiz genişlik (0, negatif veya çok büyük)",
    E2026 => "Dizi boyutu sınır aşımı",
    E2027 => "Döngü açma sınırı aşıldı",
    E2028 => "Geçersiz aralık (end < start)",
    E2029 => "Sabit dizi indeksi sınır dışı",

    // ─── Saat/sıfırlama alanları (domain-inference.md) ───
    E3001 => "Saat alanı uyumsuzluğu (CDC)",
    E3002 => "Tanımsız saat alanı",
    E3003 => "Sıfırlama alanı uyumsuzluğu (RDC)",
    E3004 => "Sıfırlama sekans ihlali",
    E3005 => "Koşullu sıfırlama karşılanmadı",
    E3006 => "Güç alanı geçişi izolasyonsuz [V1]",
    E3007 => "Güç sekans ihlali [V1]",
    E3008 => "Retention eksik [V1]",
    E3009 => "Bilgi akışı ihlali (trust_level) [V1]",
    E3010 => "Domain belirsiz (çoklu saat, anotasyon yok)",
    E3011 => "Register birden fazla domainden yazılıyor",
    E3012 => "'on' bloğunda yabancı domain sinyali okunuyor",

    // ─── Bağlantı/sürücü (type-inference.md) ───
    E4001 => "Çift sürücü",
    E4002 => "Sürücüsüz çıkış portu",
    E4003 => "Lineer port çift tüketim [V1]",
    E4004 => "Lineer port tüketilmedi [V1]",

    // ─── Bütçe ve zamanlama kontratları ───
    E6001 => "Kaynak bütçesi aşıldı",
    E6003 => "@false_path kanıtlanamadı (yol gerçekten var)",
    E6004 => "@multicycle pipeline derinliğiyle uyuşmuyor",

    // ─── Sürümleme ───
    E7001 => "SemVer ihlali: kırıcı değişiklik ama MAJOR bump yok",
    E7002 => "abi_version değişmeden arayüz değişti",

    // ─── Release disiplini ───
    E9001 => "todo! ile release build yapılamaz",
    E9002 => "Determinizm ihlali",

    // ─── Uyarılar ───
    W0010 => "Belirsiz operatör önceliği, parantez önerilir",
    W0020 => "Bilinmeyen nitelik (attribute)",
    W0021 => "Kullanılmayan doc yorumu",
    W1001 => "Kullanılmayan sinyal / bağlama",
    W1002 => "Gölgeleme (iç kapsamda aynı isim)",
    W1003 => "Yerleşik ismin gölgelenmesi",
    W1004 => "Yazılıp okunmayan register",
    W1005 => "Kullanılmayan import",
    W2010 => "Daraltıcı dönüşüm (bilgi kaybı)",
    W2011 => "Kullanılmayan tip parametresi",
    W2012 => "Tip belirtilmedi, varsayılan kullanıldı",
    W2020 => "Sabit koşul — dal her zaman aynı sonuç veriyor",
    W2021 => "Kullanılmayan const bildirimi",
    W3001 => "Register hiç yazılmıyor",
    W3002 => "Gereksiz sync() (aynı domain)",
    W3003 => "Çok bitli sync() — bit tutarlılığı garanti değil",
    W3004 => "Kullanılmayan domain tanımı",
    W4001 => "Kullanılmayan sinyal (_ öneki ile susturulur)",
    W4002 => "Yazılıp hiç okunmayan register",
}

impl ErrorCode {
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

//! `volt explain <KOD>` uzun açıklamaları (cli-contract.md §9).
//!
//! Her kod için yapı: başlık + özet + NEDEN SORUN + ÖRNEK + ÇÖZÜM +
//! [NOT] + DAHA FAZLA. Metinler `en.rs` / `tr.rs` içinde yaşar; iki
//! dosya da joker kolsuz `match` kullanır — yeni kod eklenince
//! derleyici iki dili birden zorlar (messages/ ile aynı disiplin).

use crate::code::ErrorCode;
use crate::messages::{self, Lang};

pub mod en;
pub mod tr;

/// cli-contract.md §9 şablonundaki tek kod açıklaması.
///
/// `example` ve `fix` girintisiz yazılır; render sırasında kod satırları
/// 2 boşluk içeri alınır ve SATIR SARILMAZ (kod bölünmemeli).
pub struct Explanation {
    /// Başlık — "E3001: <başlık>" satırının başlık kısmı.
    pub title: &'static str,
    /// Başlığın altındaki tek cümlelik özet.
    pub summary: &'static str,
    /// NEDEN SORUN — paragraflar "\n\n" ile ayrılır, sarılır.
    pub why: &'static str,
    /// ÖRNEK — hatayı üreten kod.
    pub example: &'static str,
    /// ÇÖZÜM — düzeltilmiş kod.
    pub fix: &'static str,
    /// SINIRLAR / NOT — opsiyonel ek bölüm (sarılır).
    pub note: Option<&'static str>,
    /// DAHA FAZLA — `explain_url()`e eklenen ek belge linkleri.
    pub extra_docs: &'static [&'static str],
}

impl Explanation {
    /// Not ve ek link içermeyen açıklama (en yaygın biçim).
    pub const fn new(
        title: &'static str,
        summary: &'static str,
        why: &'static str,
        example: &'static str,
        fix: &'static str,
    ) -> Self {
        Explanation {
            title,
            summary,
            why,
            example,
            fix,
            note: None,
            extra_docs: &[],
        }
    }

    /// SINIRLAR / NOT bölümü ekler.
    pub const fn with_note(mut self, note: &'static str) -> Self {
        self.note = Some(note);
        self
    }

    /// DAHA FAZLA bölümüne ek belge linkleri ekler.
    pub const fn with_docs(mut self, docs: &'static [&'static str]) -> Self {
        self.extra_docs = docs;
        self
    }
}

/// Kod açıklaması — dil dağıtıcısı (messages::message ile aynı desen).
pub fn explanation(lang: Lang, code: ErrorCode) -> Explanation {
    match lang {
        Lang::En => en::explanation(code),
        Lang::Tr => tr::explanation(code),
    }
}

/// Bölüm başlıkları (§9: TR "NEDEN SORUN / ÖRNEK / ÇÖZÜM / DAHA FAZLA").
struct SectionHeaders {
    why: &'static str,
    example: &'static str,
    fix: &'static str,
    note: &'static str,
    more: &'static str,
}

fn headers(lang: Lang) -> SectionHeaders {
    match lang {
        Lang::En => SectionHeaders {
            why: "WHY THIS IS A PROBLEM",
            example: "EXAMPLE",
            fix: "SOLUTION",
            note: "NOTE",
            more: "FOR MORE",
        },
        Lang::Tr => SectionHeaders {
            why: "NEDEN SORUN",
            example: "ÖRNEK",
            fix: "ÇÖZÜM",
            note: "NOT",
            more: "DAHA FAZLA",
        },
    }
}

/// En dar mantıklı sarma genişliği — daha darı okunmaz hale getirir.
pub const MIN_WIDTH: usize = 40;

/// Varsayılan terminal genişliği (cli-contract.md §9, 80 sütun).
pub const DEFAULT_WIDTH: usize = 80;

/// ANSI kalın+camgöbeği başlık; `color=false` iken düz metin.
const TITLE_STYLE: &str = "\x1b[1;36m";
const HEADER_STYLE: &str = "\x1b[1m";
const RESET_STYLE: &str = "\x1b[0m";

/// §9 şablonuna göre tam açıklama metni üretir (stdout verisi, §11).
///
/// `width` sütun sayısına sarar (alt sınır [`MIN_WIDTH`]); kod blokları
/// sarılmaz. `color` yalnızca başlık ve bölüm başlıklarını boyar.
pub fn render_explanation(code: ErrorCode, lang: Lang, width: usize, color: bool) -> String {
    let width = width.max(MIN_WIDTH);
    let exp = explanation(lang, code);
    let hdr = headers(lang);
    let paint = |style: &str, text: &str| {
        if color {
            format!("{style}{text}{RESET_STYLE}")
        } else {
            text.to_string()
        }
    };

    let mut out = String::new();
    out.push_str(&paint(
        TITLE_STYLE,
        &format!("{}: {}", code.as_str(), exp.title),
    ));
    out.push_str("\n\n");
    out.push_str(&wrap(exp.summary, width));
    out.push_str("\n\n");

    out.push_str(&paint(HEADER_STYLE, hdr.why));
    out.push_str("\n\n");
    out.push_str(&wrap(exp.why, width));
    out.push_str("\n\n");

    out.push_str(&paint(HEADER_STYLE, hdr.example));
    out.push_str("\n\n");
    out.push_str(&indent_code(exp.example));
    out.push_str("\n\n");

    out.push_str(&paint(HEADER_STYLE, hdr.fix));
    out.push_str("\n\n");
    out.push_str(&indent_code(exp.fix));
    out.push_str("\n\n");

    if let Some(note) = exp.note {
        out.push_str(&paint(HEADER_STYLE, hdr.note));
        out.push_str("\n\n");
        out.push_str(&wrap(note, width));
        out.push_str("\n\n");
    }

    out.push_str(&paint(HEADER_STYLE, hdr.more));
    out.push('\n');
    out.push_str(&format!("  {}\n", code.explain_url()));
    for doc in exp.extra_docs {
        out.push_str(&format!("  {doc}\n"));
    }
    out
}

/// `volt explain --list` çıktısı: kodlar kategori başlıkları altında,
/// kısa açıklamalarıyla (aktif kısa mesaj tablosundan) listelenir.
pub fn render_list(lang: Lang) -> String {
    let mut out = String::new();
    let mut current = "";
    for &code in ErrorCode::ALL {
        let cat = category_name(lang, code);
        if cat != current {
            if !current.is_empty() {
                out.push('\n');
            }
            out.push_str(cat);
            out.push('\n');
            current = cat;
        }
        out.push_str(&format!(
            "  {}  {}\n",
            code.as_str(),
            messages::message(lang, code)
        ));
    }
    out
}

/// Kodun kategori başlığı (--list gruplaması). Aralıklar code.rs'teki
/// blok yorumlarıyla birebir: E2001-E2019 tip, E2020+ sabit değerlendirme.
pub fn category_name(lang: Lang, code: ErrorCode) -> &'static str {
    let s = code.as_str();
    let num: u32 = s[1..].parse().unwrap_or(0);
    if s.starts_with('W') {
        return match lang {
            Lang::En => "Warnings",
            Lang::Tr => "Uyarılar",
        };
    }
    match (lang, num) {
        (Lang::En, 1..=999) => "Syntax",
        (Lang::En, 1000..=1999) => "Name resolution",
        (Lang::En, 2000..=2019) => "Type inference",
        (Lang::En, 2020..=2999) => "Constant evaluation",
        (Lang::En, 3000..=3999) => "Clock/reset domains",
        (Lang::En, 4000..=4999) => "Connectivity/drivers",
        (Lang::En, 5000..=5999) => "Behavioral contracts",
        (Lang::En, 6000..=6999) => "Budget and timing contracts",
        (Lang::En, 7000..=7999) => "Versioning",
        (Lang::En, _) => "Release discipline",
        (Lang::Tr, 1..=999) => "Sözdizimi",
        (Lang::Tr, 1000..=1999) => "İsim çözümleme",
        (Lang::Tr, 2000..=2019) => "Tip çıkarımı",
        (Lang::Tr, 2020..=2999) => "Sabit değerlendirme",
        (Lang::Tr, 3000..=3999) => "Saat/sıfırlama alanları",
        (Lang::Tr, 4000..=4999) => "Bağlantı/sürücü",
        (Lang::Tr, 5000..=5999) => "Davranışsal kontratlar",
        (Lang::Tr, 6000..=6999) => "Bütçe ve zamanlama kontratları",
        (Lang::Tr, 7000..=7999) => "Sürümleme",
        (Lang::Tr, _) => "Release disiplini",
    }
}

/// Bilinmeyen giriş için en yakın kodu önerir (Levenshtein ≤ 2).
pub fn suggest(input: &str) -> Option<ErrorCode> {
    let target = input.trim().to_ascii_uppercase();
    ErrorCode::ALL
        .iter()
        .copied()
        .map(|c| (levenshtein(c.as_str(), &target), c))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

/// Klasik iki-satırlı DP; kod metinleri ASCII olduğundan bayt bazlı.
fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Paragrafları koruyarak kelime sınırından sarar. Genişlik ölçümü
/// karakter sayısıdır — Türkçe aksanlı harfler tek karakter sayılır.
fn wrap(text: &str, width: usize) -> String {
    let mut out = String::new();
    for (i, paragraph) in text.split("\n\n").enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        let mut line_len = 0usize;
        for word in paragraph.split_whitespace() {
            let word_len = word.chars().count();
            if line_len == 0 {
                out.push_str(word);
                line_len = word_len;
            } else if line_len + 1 + word_len <= width {
                out.push(' ');
                out.push_str(word);
                line_len += 1 + word_len;
            } else {
                out.push('\n');
                out.push_str(word);
                line_len = word_len;
            }
        }
    }
    out
}

/// Kod bloğunu 2 boşluk içeri alır; satırlar olduğu gibi korunur.
fn indent_code(code: &str) -> String {
    code.lines()
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("  {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

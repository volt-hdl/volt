//! Register haritası tutarlılık denetimi (ADR-0063).
//!
//! İki soru yanıtlanır:
//!
//! 1. **Seviye 1** (`volt build --check-regmap`): üretilen sürücü, üretilen
//!    RTL'in adres çözümlemesiyle uyuşuyor mu? → [`check_rtl`].
//! 2. **Seviye 2** (`volt check-regmap <tasarım> --against <dosya>`): elde
//!    duran (belki bayat, belki elle düzenlenmiş) bir sürücü dosyası
//!    tasarımın bugünkü haritasıyla uyuşuyor mu? → [`check_file`].
//!
//! Her iki yol da dosyayı ortak bir [`DriverView`]'a indirger: sürücünün
//! yazılıma söz verdiği olgular (taban adres, register adı/offset'i/erişimi/
//! maskesi/reset değeri, alan adı/konumu/maskesi/davranışı). Yorumlar,
//! boşluk ve sıra bu görünüme girmez — bu yüzden biçim farkı ayrışma
//! sayılmaz. Yalnız Volt'un ürettiği biçim okunur ([`parse`]); elle yazılmış
//! bir başlık [`Unsupported`] ile açıkça reddedilir.

mod code;
mod decls;
mod diff;
mod hash;
mod header;
mod parse_c;
mod parse_json;
mod parse_rust;
mod rtl;

use std::path::Path;

use volt_ast::mmio::{FieldDesc, RegAccess, RegDesc, RegMap};

use crate::names::upper_snake;
use crate::{EmitOpts, SwKind};

pub use diff::{diff, Drift, DriftKind};
pub use hash::{regmap_hash, view_hash};
pub use header::Header;
pub use rtl::check_rtl;

/// Üretilen `.h`/`.rs` dosyasının iki imza satırı (ADR-0063 §4).
pub(crate) fn signature(map: &RegMap, opts: &EmitOpts) -> String {
    header::render(&opts.version, &opts.source, &regmap_hash(map))
}

/// `--against` dosyasının biçimi — uzantıdan seçilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `.h` — `--emit=c`.
    C,
    /// `.rs` — `--emit=rust`.
    Rust,
    /// `.json` — `--emit=regmap`.
    Json,
}

impl Format {
    /// `.h` / `.rs` / `.json`; başka uzantı `None`.
    pub fn from_path(path: &Path) -> Option<Format> {
        match path.extension()?.to_str()? {
            "h" => Some(Format::C),
            "rs" => Some(Format::Rust),
            "json" => Some(Format::Json),
            _ => None,
        }
    }

    /// Bu biçimi üreten yazılım çıktısı türü.
    pub fn kind(self) -> SwKind {
        match self {
            Format::C => SwKind::C,
            Format::Rust => SwKind::Rust,
            Format::Json => SwKind::Json,
        }
    }

    /// Sürücü türünden biçim (`regmap-md` denetlenmez → `None`).
    pub fn of_kind(kind: SwKind) -> Option<Format> {
        match kind {
            SwKind::C => Some(Format::C),
            SwKind::Rust => Some(Format::Rust),
            SwKind::Json => Some(Format::Json),
            SwKind::Markdown => None,
        }
    }
}

/// Sürücünün yazılıma verdiği sözlerin biçimden bağımsız özeti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverView {
    /// Modül adı, `UPPER_SNAKE` (`PWM_REGS`) — C önekiyle aynı biçim.
    pub module: String,
    pub base: u64,
    /// Dosyadaki (ya da haritadaki) bildirim sırası.
    pub registers: Vec<RegView>,
}

/// Tek register'ın sözleşmesi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegView {
    /// `UPPER_SNAKE` (`DATA_OUT`).
    pub name: String,
    pub offset: u64,
    /// Okuma erişimcisi var mı (`WriteOnly` değil).
    pub readable: bool,
    /// Yazma erişimcisi var mı (`ReadOnly` değil).
    pub writable: bool,
    /// Adlandırılmış alan bitleri.
    pub mask: u32,
    /// Sıfırlama sonrası değer (ADR-0044: hep 0).
    pub reset: u32,
    /// Adlandırılmış alanlar; `@reserved` dolgu yok (maskede görünür).
    pub fields: Vec<FieldView>,
}

impl RegView {
    /// `ReadWrite` | `ReadOnly` | `WriteOnly` | `none`.
    pub fn access_name(&self) -> &'static str {
        access_name(self.readable, self.writable)
    }
}

/// `(okunur, yazılır)` çiftinin Volt erişim adı.
pub fn access_name(readable: bool, writable: bool) -> &'static str {
    match (readable, writable) {
        (true, true) => "ReadWrite",
        (true, false) => "ReadOnly",
        (false, true) => "WriteOnly",
        (false, false) => "none",
    }
}

/// Tek alanın sözleşmesi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldView {
    /// `UPPER_SNAKE`.
    pub name: String,
    pub lsb: u32,
    /// Kaydırılmamış alan maskesi (`bits<3>` → `0x7`).
    pub mask: u32,
    pub behavior: FieldBehavior,
}

/// Alanın yazma davranışı — sürücüde ayrı erişimci doğurur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldBehavior {
    /// Oku/yaz (register erişimine göre).
    Normal,
    /// `@self_clearing` → `trigger_*`.
    SelfClearing,
    /// `@w1c` → `clear_*`.
    W1c,
}

impl FieldBehavior {
    pub fn name(self) -> &'static str {
        match self {
            FieldBehavior::Normal => "normal",
            FieldBehavior::SelfClearing => "self-clearing",
            FieldBehavior::W1c => "W1C",
        }
    }

    fn of(field: &FieldDesc) -> FieldBehavior {
        if field.self_clearing {
            FieldBehavior::SelfClearing
        } else if field.w1c {
            FieldBehavior::W1c
        } else {
            FieldBehavior::Normal
        }
    }
}

/// Register'ın sıfırlama değeri. ADR-0044: `@mmio` register'ları hep 0'a
/// sıfırlanır; sabit kimlik sözcüğü geldiğinde (ADR-0053 "Ertelenen")
/// değer `RegDesc`'ten okunacak.
pub fn reset_value(_reg: &RegDesc) -> u32 {
    0
}

/// Kaydırılmamış alan maskesi.
pub(crate) fn field_mask(width: u32) -> u32 {
    if width >= 32 {
        u32::MAX
    } else {
        (1u32 << width) - 1
    }
}

/// Haritanın sürücü görünümü — denetimin "RTL" tarafı. Harita, RTL'in
/// üretildiği bilginin kendisidir (parser `desugar_mmio`); Seviye 1 bu
/// görünümün üretilen SV ile uyuştuğunu ayrıca doğrular.
pub fn view_of(map: &RegMap) -> DriverView {
    DriverView {
        module: upper_snake(&map.module),
        base: map.base,
        registers: map
            .registers
            .iter()
            .map(|r| RegView {
                name: upper_snake(&r.name),
                offset: r.offset,
                readable: r.access != RegAccess::WriteOnly,
                writable: r.access != RegAccess::ReadOnly,
                mask: r.read_mask(),
                reset: reset_value(r),
                fields: r
                    .named()
                    .map(|f| FieldView {
                        name: upper_snake(&f.name),
                        lsb: f.lsb,
                        mask: field_mask(f.width),
                        behavior: FieldBehavior::of(f),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Dosya Volt'un ürettiği desteklenen bir biçim değil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsupported {
    /// Uzantı `.h`/`.rs`/`.json` değil.
    Extension(String),
    /// `Generated by Volt` imzası yok — elle yazılmış ya da başka araç.
    NotVolt,
    /// Volt üretimi ama `regmap-hash` imzasından önceki bir sürüm.
    OldVolt,
    /// İmza var ama zorunlu bir parça (ör. taban adres) okunamadı.
    Malformed(String),
}

/// Ayrıştırılmış dosya.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub header: Header,
    pub view: DriverView,
    /// Dosyanın kendi içindeki çelişkiler (ör. `_OFFSET` sabiti ile adres
    /// makrosu ayrı değer söylüyor) — el düzenlemesinin tipik izi.
    pub inconsistencies: Vec<Drift>,
}

/// Volt'un ürettiği `.h`/`.rs`/`.json` metnini görünüme indirger.
pub fn parse(format: Format, text: &str) -> Result<Parsed, Unsupported> {
    parse_with(format, text, None)
}

/// [`parse`] + beklenen görünüm: C/Rust'ta tek başına belirsiz bir alan
/// sabiti adını (`IRQ_STATUS_RX_SHIFT`) tasarımdaki bölünmeyle çözer.
/// Baştaki UTF-8 BOM (Windows düzenleyicileri) biçim farkıdır, atlanır.
pub fn parse_with(
    format: Format,
    text: &str,
    hint: Option<&DriverView>,
) -> Result<Parsed, Unsupported> {
    let body = without_bom(text);
    let bom = text.len() - body.len();
    let mut parsed = match format {
        Format::C => parse_c::parse(body, hint),
        Format::Rust => parse_rust::parse(body, hint),
        Format::Json => parse_json::parse(body),
    }?;
    let (s, e) = parsed.header.span;
    parsed.header.span = (s + bom, e + bom);
    Ok(parsed)
}

fn without_bom(text: &str) -> &str {
    text.strip_prefix(|c: char| u32::from(c) == 0xFEFF)
        .unwrap_or(text)
}

/// Ayrışmanın olası nedeni — dosyadaki beyan edilen hash'e göre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// Dosya eski bir haritadan üretilmiş, sonra dokunulmamış.
    Stale,
    /// Dosya bugünkü haritadan üretilmiş, sonra elle değiştirilmiş.
    Edited,
    /// Eski haritadan üretilmiş VE elle değiştirilmiş.
    StaleAndEdited,
}

/// Bir dosyanın denetim sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub format: Format,
    pub parsed: Parsed,
    /// Tasarımın bugünkü harita hash'i.
    pub design_hash: String,
    /// Dosyanın içeriğinden (görünümünden) yeniden hesaplanan hash.
    pub content_hash: String,
    /// Boşsa dosya tasarımla uyumlu.
    pub drift: Vec<Drift>,
    /// Görünüm hash'i tasarımınkine eşitti — ayrıntılı fark listesi
    /// gerekmedi (yalnız erişimci kodu karşılaştırıldı).
    pub fast_path: bool,
    /// Erişimci kodu (yorumsuz belirteçler) aynı sürümün üretimiyle
    /// karşılaştırıldı mı? Farklı Volt sürümünün dosyasında `false`.
    pub code_compared: bool,
    pub cause: Option<Cause>,
}

impl Report {
    pub fn is_match(&self) -> bool {
        self.drift.is_empty()
    }
}

/// Seviye 2: `text` dosyasını tasarımın haritalarıyla karşılaştırır.
///
/// Sıra: imza → ayrıştırma → görünüm hash'i. Hash eşitse ayrıntılı fark
/// atlanır (hızlı yol); yine de aynı sürümün ürettiği dosyada erişimci
/// kodu belirteç belirteç karşılaştırılır, çünkü beyan edilen hash satırı
/// el düzenlemesinden sonra da yerinde durur. `maps` boş olmamalıdır.
pub fn check_file(
    maps: &[RegMap],
    format: Format,
    text: &str,
    opts: &EmitOpts,
) -> Result<Report, Unsupported> {
    let module = parse(format, text)?.view.module;
    let (map, module_drift) = select_map(maps, &module);
    let expected = view_of(map);
    let parsed = parse_with(format, text, Some(&expected))?;
    let design_hash = view_hash(&expected);
    let content_hash = view_hash(&parsed.view);
    let fast_path = content_hash == design_hash && parsed.inconsistencies.is_empty();
    let mut drift: Vec<Drift> = module_drift.into_iter().collect();
    if !fast_path {
        drift.extend(parsed.inconsistencies.iter().cloned());
        drift.extend(diff(&expected, &parsed.view));
    }
    let code_compared = drift.is_empty() && parsed.header.version == opts.version;
    if code_compared {
        let generated = format.kind().render(map, opts);
        drift.extend(code::compare(
            format,
            &expected.module,
            without_bom(text),
            &generated,
        ));
    }
    let cause = if drift.is_empty() {
        None
    } else if parsed.header.hash == design_hash {
        Some(Cause::Edited)
    } else if parsed.header.hash == content_hash && parsed.inconsistencies.is_empty() {
        Some(Cause::Stale)
    } else {
        Some(Cause::StaleAndEdited)
    };
    Ok(Report {
        format,
        parsed,
        design_hash,
        content_hash,
        drift,
        fast_path,
        code_compared,
        cause,
    })
}

/// Dosyanın modülüne karşılık gelen harita. Tek harita varsa ad farkı bir
/// ayrışma kalemidir; birden çoksa ve hiçbiri eşleşmezse ilki seçilir ve
/// kalem mevcut modülleri listeler.
fn select_map<'a>(maps: &'a [RegMap], module: &str) -> (&'a RegMap, Option<Drift>) {
    if let Some(m) = maps.iter().find(|m| upper_snake(&m.module) == module) {
        return (m, None);
    }
    let names: Vec<String> = maps.iter().map(|m| upper_snake(&m.module)).collect();
    let drift = Drift {
        subject: module.to_string(),
        kind: DriftKind::Module,
        file: Some(module.to_string()),
        rtl: Some(names.join(", ")),
    };
    (&maps[0], Some(drift))
}

/// `0x0C` — offset gösterimi (üreticilerle aynı).
pub(crate) fn hex_offset(v: u64) -> String {
    format!("0x{v:02X}")
}

/// `0x0000001F` — 32 bitlik sözcük gösterimi.
pub(crate) fn hex_word(v: u64) -> String {
    format!("0x{v:08X}")
}

/// Ad eşlemesi için normal biçim: büyük harf, alt çizgisiz. C/Rust
/// erişimci adları ham Volt adını (`data_out`), sabitler `UPPER_SNAKE`'i
/// taşır; ikisi bu biçimde buluşur.
pub(crate) fn norm(parts: &[&str]) -> String {
    parts
        .iter()
        .flat_map(|p| p.chars())
        .filter(|c| *c != '_')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// `0x4001_0000U` / `16U` / `0x7` → sayı (C ve Rust literal'leri).
pub(crate) fn parse_int(text: &str) -> Option<u64> {
    let t = text
        .trim()
        .trim_matches(|c| c == '(' || c == ')')
        .trim_end_matches(['U', 'u', 'L', 'l'])
        .replace('_', "");
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        t.parse().ok()
    }
}

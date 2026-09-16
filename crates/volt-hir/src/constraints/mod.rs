//! Zamanlama kısıtı modeli (ADR-0054): domain bildirimleri, saat
//! portları, üretilen CDC köprüleri ve `@timing` / `@false_path` /
//! `@multicycle` niteliklerinden araçtan bağımsız bir kısıt modeli
//! kurar. Metin üretimi (`.sdc` / `.xdc`) `volt-sdc-emit`'tedir; bu
//! modül yalnız ANLAM taşır ve iki tanı üretir:
//!
//! * **E0017** — desteklenmeyen ya da tutarsız kısıt biçimi (bilinmeyen
//!   sinyal, birimsiz gecikme, domain frekansıyla çelişen `@timing`).
//!   Her zaman koşar (`volt check`, LSP, `analyze`): yanlış yazılmış bir
//!   kısıt sessizce yok sayılamaz (UX Anayasası).
//! * **W0022** — saat portunun alanında `frequency` yok, `create_clock`
//!   üretilmedi. Yalnız kısıt dosyası İSTENDİĞİNDE (`--emit=sdc,xdc`)
//!   raporlanır: SDC istemeyen bir tasarımcıya frekans dayatılmaz.
//!
//! Model çözümleme sonucuna bağlı değildir: portlar, register'lar,
//! wire/let'ler ve örnekler modül gövdesinden ada göre toplanır; alt
//! modül örnekleri `örnek/` öneki ile düzleştirilir (Vivado / DC
//! hiyerarşik hücre adı biçimi).

mod parse;
mod walk;

use std::collections::HashSet;

use volt_ast::{ClockEdge, SourceFile};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

pub use parse::{format_ps, PathKind};

/// Pikosaniye cinsinden bir saniye — periyot hesabı için.
pub const PS_PER_SECOND: u64 = 1_000_000_000_000;

/// Bir kısıtın uç noktası: üretilen SV'deki ad (hiyerarşik önek dâhil,
/// `fb/mem`). Üretici lehçesi bunu `get_ports` / `get_cells` /
/// `get_clocks` çağrısına çevirir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Modül portu (`get_ports`).
    Port(String),
    /// Register (`get_cells {ad_reg*}` — Vivado/DC `_reg` adlandırması).
    Cells(String),
    /// Alt modül örneğinin portu (`get_pins {örnek/port}`).
    Pin(String),
    /// Adlı saat (`get_clocks`).
    Clock(String),
}

impl Target {
    pub fn name(&self) -> &str {
        match self {
            Target::Port(n) | Target::Cells(n) | Target::Pin(n) | Target::Clock(n) => n,
        }
    }
}

/// Modülün bir saat portu ve alanının bilinen zamanlama bilgisi.
#[derive(Debug, Clone)]
pub struct ClockConstraint {
    /// Saat portunun adı — `create_clock -name` ve `get_ports`.
    pub port: String,
    /// Alan adı: `@Ad` anotasyonu ya da anotasyonsuz portta portun adı (K2).
    pub domain: String,
    pub edge: ClockEdge,
    /// Hz; domain `frequency` ya da `@timing(port = F)` / `>= F`'ten.
    pub freq_hz: Option<u64>,
    /// Frekansın kaynağı (yorum satırı için).
    pub freq_source: Option<FreqSource>,
    /// Port bildirimi.
    pub span: Span,
    /// Açık domain bildiriminin adı konumu (W0022 buraya bağlanır).
    pub domain_span: Option<Span>,
}

impl ClockConstraint {
    /// Periyot, pikosaniye (en yakına yuvarlanır); frekans yoksa `None`.
    pub fn period_ps(&self) -> Option<u64> {
        self.freq_hz
            .filter(|&f| f > 0)
            .map(|f| (PS_PER_SECOND * 2 / f).div_ceil(2))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreqSource {
    /// `domain D { frequency = ... }`
    Domain,
    /// `@timing(clk = F)` ya da `@timing(clk >= F)` (alan frekansı yok).
    Timing,
}

/// Tek bir yol kısıtı (`set_false_path`, `set_max_delay`, ...).
#[derive(Debug, Clone)]
pub struct PathRule {
    pub kind: PathKind,
    pub from: Option<Target>,
    pub to: Option<Target>,
    /// Kaynak açıklaması — üretilen dosyada yorum satırı.
    pub comment: String,
}

/// Üretilen bir CDC köprüsü (sync(), AsyncFifo, ...): kuralları ve
/// XDC `ASYNC_REG` özniteliği alacak senkronizatör register'ları.
#[derive(Debug, Clone)]
pub struct Bridge {
    /// Kısa tür adı: `sync`, `sync3`, `AsyncFifo`, ...
    pub kind: &'static str,
    /// Üretilen SV'deki temel ad (hiyerarşik önek dâhil).
    pub name: String,
    /// Kaynak / hedef saat portu (üst modülün adı); bilinmiyorsa `None`.
    pub from_clock: Option<String>,
    pub to_clock: Option<String>,
    pub rules: Vec<PathRule>,
    /// Senkronizatör zinciri register'ları (hiyerarşik ad, `_reg` eksiz).
    pub async_regs: Vec<String>,
}

/// Bir modülün (üst modül kabul edilerek) tüm kısıtları.
#[derive(Debug, Clone)]
pub struct ModuleConstraints {
    pub module: String,
    pub clocks: Vec<ClockConstraint>,
    /// Asenkron saat grupları: alan başına bir grup, gruptaki saat
    /// portu adları. Tek grup varsa `set_clock_groups` üretilmez.
    pub groups: Vec<Vec<String>>,
    pub bridges: Vec<Bridge>,
    /// Kullanıcı nitelikleri (`@timing(max_delay)`, `@false_path`,
    /// `@multicycle`) — kaynak sırasında.
    pub paths: Vec<PathRule>,
}

impl ModuleConstraints {
    /// Frekansı bilinen saat portları (yalnız bunlar `create_clock` alır).
    pub fn defined_clocks(&self) -> impl Iterator<Item = &ClockConstraint> {
        self.clocks.iter().filter(|c| c.freq_hz.is_some())
    }

    /// `set_clock_groups`'a giren gruplar: en az bir saati tanımlı
    /// olanlar; iki gruptan az kaldıysa boş.
    pub fn async_groups(&self) -> Vec<Vec<String>> {
        let groups: Vec<Vec<String>> = self
            .groups
            .iter()
            .map(|g| {
                g.iter()
                    .filter(|p| {
                        self.clocks
                            .iter()
                            .any(|c| &c.port == *p && c.freq_hz.is_some())
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .filter(|g| !g.is_empty())
            .collect();
        if groups.len() < 2 {
            Vec::new()
        } else {
            groups
        }
    }
}

/// `collect_constraints` çıktısı.
#[derive(Debug, Default)]
pub struct ConstraintResult {
    /// En az bir saat portu olan her modül için bir giriş (kaynak sırası).
    pub modules: Vec<ModuleConstraints>,
    /// E0017 tanıları — her zaman raporlanır.
    pub diagnostics: Vec<Diagnostic>,
}

impl ConstraintResult {
    /// W0022: frekansı bilinmeyen her alan için bir uyarı (alan
    /// bildirimi ya da anotasyonsuz saat portu başına BİR kez). Yalnız
    /// kısıt dosyası istendiğinde çağrılır.
    pub fn missing_frequency_warnings(&self) -> Vec<Diagnostic> {
        let mut seen: HashSet<(u32, u32, u32)> = HashSet::new();
        let mut out = Vec::new();
        for m in &self.modules {
            for c in m.clocks.iter().filter(|c| c.freq_hz.is_none()) {
                let anchor = c.domain_span.unwrap_or(c.span);
                if !seen.insert((anchor.file.0, anchor.start, anchor.end)) {
                    continue;
                }
                out.push(missing_frequency(c, &m.module));
            }
        }
        out
    }
}

fn missing_frequency(c: &ClockConstraint, module: &str) -> Diagnostic {
    let (message, label) = if c.domain_span.is_some() {
        (
            lstr!(en: "domain '{}' has no frequency; no create_clock emitted for '{}'", c.domain, c.port;
                  tr: "'{}' alanının frekansı yok; '{}' için create_clock üretilmedi", c.domain, c.port),
            lstr!(en: "add 'frequency = ...' to this domain"; tr: "bu alana 'frequency = ...' ekleyin"),
        )
    } else {
        (
            lstr!(en: "clock port '{}' has no frequency; no create_clock emitted", c.port;
                  tr: "'{}' saat portunun frekansı yok; create_clock üretilmedi", c.port),
            lstr!(en: "no domain declares a frequency for this clock"; tr: "bu saat için frekans bildiren bir alan yok"),
        )
    };
    let mut diag = Diagnostic::warning(
        ErrorCode::W0022,
        message,
        LabeledSpan::primary(c.domain_span.unwrap_or(c.span), label),
        lstr!(en: "declare it in the domain ('domain {} {{ clock = posedge, frequency = 100.mhz }}') or on the module ('@timing({} = 100.mhz)')", c.domain, c.port;
              tr: "alanda bildirin ('domain {} {{ clock = posedge, frequency = 100.mhz }}') ya da modülde ('@timing({} = 100.mhz)')", c.domain, c.port),
    )
    .with_note(
        NoteKind::Reason,
        lstr!(en: "a create_clock needs a period; the timing tool will treat '{}' as unconstrained", c.port;
              tr: "create_clock bir periyot ister; zamanlama aracı '{}' portunu kısıtsız sayacak", c.port),
    );
    if c.domain_span.is_some() {
        diag = diag.with_note(
            NoteKind::Note,
            lstr!(en: "module '{module}' uses this domain on port '{}'", c.port;
                  tr: "'{module}' modülü bu alanı '{}' portunda kullanıyor", c.port),
        );
    }
    diag
}

/// Birimdeki her saatli modül için kısıt modeli kurar; E0017 tanıları
/// `diagnostics`'te döner.
pub fn collect_constraints(ast: &SourceFile) -> ConstraintResult {
    walk::collect(ast)
}

/// Yalnız tanılar (sürücü `check`, LSP ve `analyze` için kısayol).
pub fn check_constraints(ast: &SourceFile) -> Vec<Diagnostic> {
    collect_constraints(ast).diagnostics
}

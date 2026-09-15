//! Yazılım tarafı üreticileri (ADR-0053): `@mmio` register haritasından
//! Rust sürücüsü, C başlığı, `regmap.json` ve Markdown belge.
//!
//! Girdi `volt_ast::mmio::RegMap`'tir — parser'ın RTL'e açtığı
//! bilginin dil bağımsız kopyası. Üreticiler saf fonksiyondur: dosya
//! sistemi, tanı ya da AST erişimi yoktur; sürücü (volt-driver) yolu
//! seçer ve yazar. Dört çıktı da aynı haritadan geldiği için birbiriyle
//! ve RTL ile tanım gereği tutarlıdır; `volt-driver/tests/sw_emit_tests.rs`
//! bunu üretilen SV'nin adres çözümlemesiyle karşılaştırarak doğrular.

mod c;
mod json;
mod markdown;
mod names;
mod rust;

use std::path::{Path, PathBuf};

use volt_ast::mmio::RegMap;

pub use names::file_stem;

/// Üretilen dosyaların başlığına giren bilgi.
#[derive(Debug, Clone)]
pub struct EmitOpts {
    /// Ana kaynak dosyanın adı (`gpio.volt`).
    pub source: String,
    /// Volt sürümü (`0.1.0`).
    pub version: String,
}

/// `--emit` yazılım çıktısı türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwKind {
    /// `build/sw/<modül>.rs` — `no_std` Rust sürücüsü.
    Rust,
    /// `build/sw/<modül>.h` — C başlığı (makro + `static inline`).
    C,
    /// `build/sw/<modül>.json` — `volt-regmap/1` şeması.
    Json,
    /// `build/docs/<modül>.md` — register haritası tabloları.
    Markdown,
}

impl SwKind {
    /// `--emit` değeri (cli-contract.md §5).
    pub fn flag(self) -> &'static str {
        match self {
            SwKind::Rust => "rust",
            SwKind::C => "c",
            SwKind::Json => "regmap",
            SwKind::Markdown => "regmap-md",
        }
    }

    /// `build/` altındaki alt dizin (cli-contract.md §4).
    pub fn subdir(self) -> &'static str {
        match self {
            SwKind::Rust | SwKind::C | SwKind::Json => "sw",
            SwKind::Markdown => "docs",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            SwKind::Rust => "rs",
            SwKind::C => "h",
            SwKind::Json => "json",
            SwKind::Markdown => "md",
        }
    }

    /// `target_dir/<subdir>/<snake_case modül>.<uzantı>`.
    pub fn output_path(self, target_dir: &Path, map: &RegMap) -> PathBuf {
        target_dir.join(self.subdir()).join(format!(
            "{}.{}",
            file_stem(&map.module),
            self.extension()
        ))
    }

    /// Haritayı bu türde metne çevirir.
    pub fn render(self, map: &RegMap, opts: &EmitOpts) -> String {
        match self {
            SwKind::Rust => rust::emit(map, opts),
            SwKind::C => c::emit(map, opts),
            SwKind::Json => json::emit(map, opts),
            SwKind::Markdown => markdown::emit(map, opts),
        }
    }
}

/// `no_std` Rust sürücüsü.
pub fn emit_rust(map: &RegMap, opts: &EmitOpts) -> String {
    rust::emit(map, opts)
}

/// C başlığı.
pub fn emit_c(map: &RegMap, opts: &EmitOpts) -> String {
    c::emit(map, opts)
}

/// `regmap.json` (`volt-regmap/1`).
pub fn emit_json(map: &RegMap, opts: &EmitOpts) -> String {
    json::emit(map, opts)
}

/// Markdown register haritası.
pub fn emit_markdown(map: &RegMap, opts: &EmitOpts) -> String {
    markdown::emit(map, opts)
}

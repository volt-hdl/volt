//! Zamanlama kısıtı üreticisi (ADR-0054): `volt_hir::ModuleConstraints`
//! modelini `.sdc` (Synopsys Design Constraints — Design Compiler,
//! Quartus, OpenSTA) ya da `.xdc` (Vivado) metnine çevirir.
//!
//! Üretici saf fonksiyondur: dosya sistemi, tanı ya da AST erişimi
//! yoktur; sürücü (volt-driver) yolu seçer ve yazar. İki lehçe aynı
//! modelden üretilir; XDC farkı yalnız senkronizatör register'larına
//! `ASYNC_REG` özniteliği (Vivado'nun metastabilite yerleşim ipucu) ve
//! başlık satırıdır. Çıktı deterministiktir (tarih yok, E9002).
//!
//! Ad geleneği: register hücreleri `get_cells {ad_reg*}` (Vivado ve
//! Design Compiler `<ad>_reg` / `<ad>_reg[i]`), portlar `get_ports`,
//! alt modül örnek portları `get_pins {örnek/port}`, saatler
//! `get_clocks` — yalnız `create_clock` almış saatler.

mod render;

use std::path::{Path, PathBuf};

use volt_hir::ModuleConstraints;

pub use render::{render, syntax_check};

/// Üretilen dosya başlığına giren bilgi.
#[derive(Debug, Clone)]
pub struct EmitOpts {
    /// Ana kaynak dosyanın adı (`vga_top.volt`).
    pub source: String,
    /// Volt sürümü (`0.1.0`).
    pub version: String,
}

/// Kısıt lehçesi — `--emit=sdc` / `--emit=xdc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// Synopsys Design Constraints (Design Compiler, Quartus, OpenSTA).
    Sdc,
    /// Xilinx Design Constraints (Vivado) — SDC + `ASYNC_REG`.
    Xdc,
}

impl Dialect {
    /// `--emit` değeri (cli-contract.md §5).
    pub fn flag(self) -> &'static str {
        match self {
            Dialect::Sdc => "sdc",
            Dialect::Xdc => "xdc",
        }
    }

    pub fn extension(self) -> &'static str {
        self.flag()
    }

    /// `target_dir/constraints/<Modül>.<sdc|xdc>` — ADR-0024 ile aynı
    /// adlandırma (modül adı, `build/rtl/<Modül>.sv` gibi).
    pub fn output_path(self, target_dir: &Path, module: &str) -> PathBuf {
        target_dir
            .join("constraints")
            .join(format!("{module}.{}", self.extension()))
    }

    /// Modelin bu lehçedeki metni.
    pub fn render(self, mc: &ModuleConstraints, opts: &EmitOpts) -> String {
        render(mc, self, opts)
    }
}

//! Zamanlama kısıtı üreticisi (ADR-0054): `volt_hir::ModuleConstraints`
//! modelini `.sdc` (Synopsys Design Constraints — Design Compiler,
//! Quartus, OpenSTA) ya da `.xdc` (Vivado) metnine çevirir.
//!
//! Üretici saf fonksiyondur: dosya sistemi, tanı ya da AST erişimi
//! yoktur; sürücü (volt-driver) yolu seçer ve yazar. İki lehçe aynı
//! modelden üretilir. Çıktı deterministiktir (tarih yok, E9002).
//!
//! İki stil (ADR-0065 §4, `--sdc-style`):
//!
//! * **targeted** (varsayılan) — `set_clock_groups` YOK. Yalnız
//!   derleyicinin ürettiği senkronizörlerin ilk aşamasına giden yol ve
//!   ham reset portunun etkinleşme yolu kısıtlanır; alanlar arası başka
//!   her yol zamanlanır (senkronizörsüz geçiş ihlal olarak görünür),
//!   reset bırakma yolları recovery/removal olarak analiz edilir. Genel
//!   `.sdc` OpenSTA/PrimeTime sözlüğünü izler (gray işaretçi:
//!   `set_max_delay -ignore_clock_latency`); `.xdc` Vivado'nun
//!   `-datapath_only` ve `set_bus_skew` komutlarını kullanır.
//! * **clock-groups** — ADR-0054 çıktısı (alanlar arası
//!   `set_clock_groups -asynchronous`, köprülere `set_false_path`),
//!   başlığa bir `# Style:` uyarı satırı eklenerek.
//!
//! XDC her iki stilde senkronizör register'larına `ASYNC_REG` özniteliği
//! (Vivado'nun metastabilite yerleşim ipucu) ekler.
//!
//! Ad geleneği: register hücreleri `get_cells {ad_reg*}` (Vivado ve
//! Design Compiler `<ad>_reg` / `<ad>_reg[i]`), portlar `get_ports`,
//! alt modül örnek portları `get_pins {örnek/port}`, saatler
//! `get_clocks` — yalnız `create_clock` almış saatler.

mod render;

use std::path::{Path, PathBuf};

use volt_hir::ModuleConstraints;

pub use render::{render, syntax_check};

/// Üretilen dosya başlığına giren bilgi ve stil.
#[derive(Debug, Clone)]
pub struct EmitOpts {
    /// Ana kaynak dosyanın adı (`vga_top.volt`).
    pub source: String,
    /// Volt sürümü (`0.1.0`).
    pub version: String,
    /// Alanlar arası kısıt stili (`--sdc-style`).
    pub style: SdcStyle,
}

/// Alanlar arası kısıt stili — `--sdc-style` (ADR-0065 §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SdcStyle {
    /// Senkronizör başına hedefli kısıtlar; saat grubu yok.
    #[default]
    Targeted,
    /// ADR-0054: alanlar arası `set_clock_groups -asynchronous`.
    ClockGroups,
}

impl SdcStyle {
    /// `--sdc-style` değeri.
    pub fn flag(self) -> &'static str {
        match self {
            SdcStyle::Targeted => "targeted",
            SdcStyle::ClockGroups => "clock-groups",
        }
    }
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

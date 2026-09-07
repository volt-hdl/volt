//! SymbiYosys `.sby` yapılandırma üretimi (F4b ADIM 1).
//!
//! Varsayılan akış tek `.sv` dosyasıdır ve kontratlar GÖMÜLÜ immediate
//! assertion olarak üretilir (`SvaMode::Immediate`): hdlc/formal
//! imajındaki Yosys ile yapılan denemede ne ayrı `.sva` + `bind (.*)`
//! akışı ne de adlandırılmış `property/endproperty` blokları
//! ayrıştırılabildi (TOK_PROPERTY sözdizimi hatası); Yosys'in
//! `read -formal` desteği always bloğu içindeki immediate
//! assert/assume/cover ile sınırlıdır ve bu kalıp sorunsuz çalışır.
//! `--emit=sva` çıktısı (property blokları) ticari araçlar için
//! olduğu gibi kalır; `.sby` yalnız tek dosyayı okur.

use std::fmt;

/// cli-contract.md §2'ye giden doğrulama kipi (`volt verify --mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SbyMode {
    /// Sınırlı model denetimi — `depth` döngüye kadar arar (varsayılan).
    #[default]
    Bmc,
    /// Sınırsız kanıt (k-tümevarım).
    Prove,
    /// `cover` kontratlarına ulaşan izler üretir.
    Cover,
}

impl SbyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SbyMode::Bmc => "bmc",
            SbyMode::Prove => "prove",
            SbyMode::Cover => "cover",
        }
    }
}

/// SMT çözücüsü (`volt verify --engine`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SbyEngine {
    #[default]
    Z3,
    Boolector,
    Yices,
}

impl SbyEngine {
    pub fn as_str(&self) -> &'static str {
        match self {
            SbyEngine::Z3 => "z3",
            SbyEngine::Boolector => "boolector",
            SbyEngine::Yices => "yices",
        }
    }
}

/// `.sby` üretim seçenekleri; varsayılanlar goal/F4b sözleşmesi:
/// bmc + derinlik 20 + z3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SbyOptions {
    pub mode: SbyMode,
    pub depth: u32,
    pub engine: SbyEngine,
}

impl Default for SbyOptions {
    fn default() -> Self {
        SbyOptions {
            mode: SbyMode::Bmc,
            depth: 20,
            engine: SbyEngine::Z3,
        }
    }
}

impl fmt::Display for SbyOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} depth={} engine={}",
            self.mode.as_str(),
            self.depth,
            self.engine.as_str()
        )
    }
}

/// Tek modül için `.sby` içeriği. `sv_file` `.sby` dosyasına GÖRECELİ
/// dosya adıdır (ikisi de `build/formal/` altına yazılır);
/// `top_module` Volt modül adıdır (SV modül adıyla birebir).
pub fn sby_config(top_module: &str, sv_file: &str, opts: &SbyOptions) -> String {
    format!(
        "[options]\n\
         mode {mode}\n\
         depth {depth}\n\
         \n\
         [engines]\n\
         smtbmc {engine}\n\
         \n\
         [script]\n\
         read -formal {sv_file}\n\
         prep -top {top_module}\n\
         \n\
         [files]\n\
         {sv_file}\n",
        mode = opts.mode.as_str(),
        depth = opts.depth,
        engine = opts.engine.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_are_bmc_depth_20_z3() {
        let opts = SbyOptions::default();
        assert_eq!(opts.mode, SbyMode::Bmc);
        assert_eq!(opts.depth, 20);
        assert_eq!(opts.engine, SbyEngine::Z3);
    }

    #[test]
    fn sby_config_matches_contract_layout() {
        let text = sby_config("Counter", "counter.sv", &SbyOptions::default());
        assert!(
            text.starts_with("[options]\nmode bmc\ndepth 20\n"),
            "{text}"
        );
        assert!(text.contains("[engines]\nsmtbmc z3\n"), "{text}");
        assert!(
            text.contains("[script]\nread -formal counter.sv\nprep -top Counter\n"),
            "{text}"
        );
        assert!(text.ends_with("[files]\ncounter.sv\n"), "{text}");
    }

    #[test]
    fn sby_config_honors_mode_depth_engine() {
        let opts = SbyOptions {
            mode: SbyMode::Prove,
            depth: 40,
            engine: SbyEngine::Boolector,
        };
        let text = sby_config("Uart", "uart.sv", &opts);
        assert!(text.contains("mode prove\n"), "{text}");
        assert!(text.contains("depth 40\n"), "{text}");
        assert!(text.contains("smtbmc boolector\n"), "{text}");
    }

    #[test]
    fn cover_mode_and_yices_engine_spell_correctly() {
        assert_eq!(SbyMode::Cover.as_str(), "cover");
        assert_eq!(SbyEngine::Yices.as_str(), "yices");
    }
}

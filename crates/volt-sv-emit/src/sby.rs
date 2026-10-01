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

/// SMT çözücüsü (`volt verify --engine`). Varsayılan boolector
/// (ADR-0082): ölçümde z3'ün 2-4 katı hızlı, zaman aşımı en az olanlardan
/// ve her kurulum yolunda (apt, hdlc/formal, OSS CAD Suite) mevcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SbyEngine {
    Z3,
    #[default]
    Boolector,
    Yices,
    Bitwuzla,
}

impl SbyEngine {
    pub fn as_str(&self) -> &'static str {
        match self {
            SbyEngine::Z3 => "z3",
            SbyEngine::Boolector => "boolector",
            SbyEngine::Yices => "yices",
            SbyEngine::Bitwuzla => "bitwuzla",
        }
    }
}

/// `.sby` üretim seçenekleri; varsayılanlar goal/F4b sözleşmesi:
/// bmc + derinlik 20; motor ADR-0082 ile boolector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SbyOptions {
    pub mode: SbyMode,
    pub depth: u32,
    pub engine: SbyEngine,
    /// İki+ saatli modüllerde `multiclock on` (Yosys clk2fflogic akışı,
    /// ADR-0027); tek saatli tasarımların çıktısı değişmez.
    pub multiclock: bool,
    /// sby `timeout` (saniye, görev başına; ADR-0075). `None`: satır
    /// yazılmaz, .sby çıktısı değişmez.
    pub timeout: Option<u32>,
}

impl Default for SbyOptions {
    fn default() -> Self {
        SbyOptions {
            mode: SbyMode::Bmc,
            depth: 20,
            engine: SbyEngine::default(),
            multiclock: false,
            timeout: None,
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
    // `multiclock on` yalnız gerektiğinde eklenir — tek saatli
    // tasarımların .sby çıktısı birebir korunur.
    let multiclock = if opts.multiclock {
        "multiclock on\n"
    } else {
        ""
    };
    let timeout = timeout_line(opts);
    format!(
        "[options]\n\
         mode {mode}\n\
         depth {depth}\n\
         {timeout}\
         {multiclock}\
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

/// Çok görevli `.sby` içindeki tek görev (ADR-0055): `[tasks]` satırı,
/// görevin `prep -top` hedefi ve görev-koşullu `multiclock on`.
///
/// sby görevin çalışma dizinini `<iş>_<ad>` olarak kurar (`<iş>` `.sby`
/// dosyasının kök adı); `name` bu yüzden `[A-Za-z0-9_]` ile sınırlıdır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbyTask {
    /// Görev adı (küçük harf modül adı).
    pub name: String,
    /// `prep -top` modülü — Volt modül adıyla birebir.
    pub top: String,
    /// İki+ saatli modül için `multiclock on` (ADR-0027).
    pub multiclock: bool,
    /// Bu görevde tepe olmayan, yükümlülüklü modüllerin makroları
    /// (`VOLT_SUB_<modül>`, ADR-0097): görev-koşullu `read -define` ile
    /// tanımlanır; o modüllerin `requires`/`assume`'u bu görevde `assert`.
    pub sub_defines: Vec<String>,
}

/// Bir birimdeki TÜM kontratlı modüller için tek `.sby` (ADR-0055).
///
/// Her modül bir sby görevidir; `sby -j N -f <iş>.sby` görevleri tek
/// süreçte paralel koşturur (sby'nin kendi görev döngüsü). Ortak satırlar
/// (`mode`, `depth`, `[engines]`, `read -formal`, `[files]`) görev
/// önekisiz; görevden görevi ayıran satırlar (`prep -top`, `multiclock`)
/// `<görev>: ` önekiyle yazılır. Görev sırası kaynak sırasıdır.
pub fn sby_config_tasks(
    tasks: &[SbyTask],
    sv_file: &str,
    extern_files: &[String],
    opts: &SbyOptions,
) -> String {
    let mut out = String::from("[tasks]\n");
    for task in tasks {
        out.push_str(&task.name);
        out.push('\n');
    }
    out.push_str(&format!(
        "\n[options]\nmode {}\ndepth {}\n{}",
        opts.mode.as_str(),
        opts.depth,
        timeout_line(opts)
    ));
    for task in tasks.iter().filter(|t| t.multiclock) {
        out.push_str(&format!("{}: multiclock on\n", task.name));
    }
    out.push_str(&format!(
        "\n[engines]\nsmtbmc {}\n\n[script]\n",
        opts.engine.as_str()
    ));
    // Extern gövdeleri (ADR-0076) üretilen SV'den önce okunur; içlerindeki
    // assert/assume YOK SAYILIR: bir üretici iddiası Volt kontratıymış
    // gibi raporlanır, varsayımı ise Volt kanıtlarını sessizce kısıtlardı.
    for f in extern_files {
        out.push_str(&format!("read_verilog -sv -noassert -noassume {f}\n"));
    }
    // ADR-0097: örneklerin requires/assume'u üst görevde yükümlülüktür;
    // makro `read -formal`'dan önce tanımlanmalı.
    for task in tasks.iter().filter(|t| !t.sub_defines.is_empty()) {
        out.push_str(&format!(
            "{}: read -define {}\n",
            task.name,
            task.sub_defines.join(" ")
        ));
    }
    out.push_str(&format!("read -formal {sv_file}\n"));
    for task in tasks {
        out.push_str(&format!("{}: prep -top {}\n", task.name, task.top));
    }
    out.push_str("\n[files]\n");
    for f in extern_files {
        out.push_str(&format!("{f}\n"));
    }
    out.push_str(&format!("{sv_file}\n"));
    out
}

/// `timeout N` satırı (sby görev başına süre sınırı → `DONE (TIMEOUT)`).
fn timeout_line(opts: &SbyOptions) -> String {
    opts.timeout
        .map(|secs| format!("timeout {secs}\n"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-0076: extern SV gövdeleri [files]'ta ve `read -formal` ile
    /// üretilen SV'den ÖNCE.
    #[test]
    fn extern_sources_are_listed_and_read_first() {
        let externs = ["extern_fifo.sv".to_string(), "extern_mem.sv".to_string()];
        let text = sby_config_tasks(
            &[task("m", "M", false)],
            "m.sv",
            &externs,
            &SbyOptions::default(),
        );
        assert!(
            text.contains(
                "[script]\nread_verilog -sv -noassert -noassume extern_fifo.sv\nread_verilog -sv -noassert -noassume extern_mem.sv\nread -formal m.sv\n"
            ),
            "{text}"
        );
        assert!(
            text.ends_with("[files]\nextern_fifo.sv\nextern_mem.sv\nm.sv\n"),
            "{text}"
        );
    }

    #[test]
    fn timeout_line_only_when_requested() {
        let plain = sby_config_tasks(
            &[task("m", "M", false)],
            "m.sv",
            &[],
            &SbyOptions::default(),
        );
        assert!(!plain.contains("timeout"), "{plain}");
        let opts = SbyOptions {
            timeout: Some(30),
            ..SbyOptions::default()
        };
        let text = sby_config_tasks(&[task("m", "M", false)], "m.sv", &[], &opts);
        assert!(text.contains("\ndepth 20\ntimeout 30\n"), "{text}");
        let single = sby_config("M", "m.sv", &opts);
        assert!(
            single.starts_with("[options]\nmode bmc\ndepth 20\ntimeout 30\n"),
            "{single}"
        );
    }

    #[test]
    fn default_options_are_bmc_depth_20_boolector() {
        // ADR-0082: varsayılan motor ölçümle boolector.
        let opts = SbyOptions::default();
        assert_eq!(opts.mode, SbyMode::Bmc);
        assert_eq!(opts.depth, 20);
        assert_eq!(opts.engine, SbyEngine::Boolector);
    }

    #[test]
    fn sby_config_matches_contract_layout() {
        let text = sby_config("Counter", "counter.sv", &SbyOptions::default());
        assert!(
            text.starts_with("[options]\nmode bmc\ndepth 20\n"),
            "{text}"
        );
        assert!(text.contains("[engines]\nsmtbmc boolector\n"), "{text}");
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
            engine: SbyEngine::Z3,
            multiclock: false,
            timeout: None,
        };
        let text = sby_config("Uart", "uart.sv", &opts);
        assert!(text.contains("mode prove\n"), "{text}");
        assert!(text.contains("depth 40\n"), "{text}");
        assert!(text.contains("smtbmc z3\n"), "{text}");
    }

    #[test]
    fn cover_mode_and_every_engine_spell_as_smtbmc_solver_names() {
        assert_eq!(SbyMode::Cover.as_str(), "cover");
        // yosys-smtbmc `-s <solver>` adları.
        assert_eq!(SbyEngine::Z3.as_str(), "z3");
        assert_eq!(SbyEngine::Boolector.as_str(), "boolector");
        assert_eq!(SbyEngine::Yices.as_str(), "yices");
        assert_eq!(SbyEngine::Bitwuzla.as_str(), "bitwuzla");
    }

    #[test]
    fn multiclock_off_by_default_and_absent_from_config() {
        assert!(!SbyOptions::default().multiclock);
        let text = sby_config("Counter", "counter.sv", &SbyOptions::default());
        assert!(!text.contains("multiclock"), "{text}");
    }

    #[test]
    fn multiclock_on_appears_in_options_section() {
        let opts = SbyOptions {
            multiclock: true,
            ..SbyOptions::default()
        };
        let text = sby_config("FifoBridge", "fifobridge.sv", &opts);
        assert!(
            text.starts_with("[options]\nmode bmc\ndepth 20\nmulticlock on\n"),
            "{text}"
        );
        assert!(text.contains("[engines]\nsmtbmc boolector\n"), "{text}");
    }

    fn task(name: &str, top: &str, multiclock: bool) -> SbyTask {
        SbyTask {
            name: name.to_string(),
            top: top.to_string(),
            multiclock,
            sub_defines: Vec::new(),
        }
    }

    /// ADR-0097: görevin tepesi olmayan, yükümlülüklü (requires/assume)
    /// modüllerin makroları görev-koşullu `read -define` satırıyla,
    /// `read -formal`'dan ÖNCE tanımlanır; tanımsız görev satır almaz.
    #[test]
    fn tasks_config_defines_sub_instance_macros_per_task() {
        let mut parent = task("parent", "Parent", false);
        parent.sub_defines = vec!["VOLT_SUB_Child".into(), "VOLT_SUB_Leaf".into()];
        let tasks = [task("child", "Child", false), parent];
        let text = sby_config_tasks(&tasks, "u.sv", &[], &SbyOptions::default());
        assert!(
            text.contains(
                "[script]\nparent: read -define VOLT_SUB_Child VOLT_SUB_Leaf\n\
                 read -formal u.sv\nchild: prep -top Child\nparent: prep -top Parent\n"
            ),
            "{text}"
        );
        assert!(!text.contains("child: read -define"), "{text}");
    }

    #[test]
    fn tasks_config_lists_every_module_as_a_task_in_source_order() {
        let tasks = [
            task("soctop", "SocTop", false),
            task("gpio", "Gpio", false),
            task("timer", "Timer", false),
        ];
        let text = sby_config_tasks(&tasks, "top.sv", &[], &SbyOptions::default());
        assert!(
            text.starts_with("[tasks]\nsoctop\ngpio\ntimer\n\n[options]\nmode bmc\ndepth 20\n"),
            "{text}"
        );
        assert!(text.contains("[engines]\nsmtbmc boolector\n"), "{text}");
        assert!(
            text.contains(
                "[script]\nread -formal top.sv\nsoctop: prep -top SocTop\n\
                 gpio: prep -top Gpio\ntimer: prep -top Timer\n"
            ),
            "{text}"
        );
        assert!(text.ends_with("[files]\ntop.sv\n"), "{text}");
    }

    #[test]
    fn tasks_config_multiclock_is_task_conditional() {
        let tasks = [
            task("fifobridge", "FifoBridge", true),
            task("counter", "Counter", false),
        ];
        let text = sby_config_tasks(&tasks, "u.sv", &[], &SbyOptions::default());
        assert!(
            text.contains("[options]\nmode bmc\ndepth 20\nfifobridge: multiclock on\n\n"),
            "{text}"
        );
        assert!(!text.contains("counter: multiclock"), "{text}");
    }

    #[test]
    fn tasks_config_honors_mode_depth_engine() {
        let opts = SbyOptions {
            mode: SbyMode::Cover,
            depth: 48,
            engine: SbyEngine::Yices,
            multiclock: false,
            timeout: None,
        };
        let text = sby_config_tasks(&[task("uart", "Uart", false)], "uart.sv", &[], &opts);
        assert!(text.contains("mode cover\ndepth 48\n"), "{text}");
        assert!(text.contains("smtbmc yices\n"), "{text}");
        assert!(text.contains("uart: prep -top Uart\n"), "{text}");
    }

    #[test]
    fn single_task_config_still_uses_tasks_section() {
        // Tek modüllü tasarımda da biçim aynı: çalışma dizini <iş>_<görev>.
        let text = sby_config_tasks(
            &[task("boundedcounter", "BoundedCounter", false)],
            "x.sv",
            &[],
            &SbyOptions::default(),
        );
        assert!(text.starts_with("[tasks]\nboundedcounter\n"), "{text}");
    }
}

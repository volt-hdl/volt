//! Volt CLI — cli-contract.md sözleşmesine göre.
//!
//! `build` ve `check` tam anlamsal boru hattı koşar:
//! parse → resolve → typeck (+const eval) → domain → emit.
//! Bir aşamada hata varsa SONRAKİNE GEÇİLMEZ (kaskad tanı önlemi);
//! CDC ihlali (E3001) derlemeyi durdurur — Volt'un vaadi.
//!
//! Çıkış kodları §2: 0 başarı, 1 derleme hatası, 2 kullanım hatası
//! (clap), 3 G/Ç hatası. Formatlar §5: human | json | short.

mod color;
mod doctor;
mod extern_stage;
mod interrupt;
mod new;
mod project;
mod reach;
mod regmap_check;
mod sim;
mod sim_lower;
mod sim_struct;
mod tool_backend;
mod verify;
mod verify_depth;
mod verify_jobs;
mod verify_plan;
mod verify_report;
mod watch;
mod waves;

use volt_hir::unit_load as unit;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use volt_diagnostics::{
    explain, lstr, render_human, render_short, to_json_value, Diagnostic, ErrorCode, Lang, Severity,
};
use volt_hir::FileScope;
use volt_sdc_emit::{Dialect, SdcStyle};
use volt_span::FileId;
use volt_span::SourceMap;
use volt_sv_emit::{
    ConstArrayStyle, SbyEngine, SbyMode, SbyOptions, SvModule, SvaFile, SvaMode, SvaProp,
};
use volt_sw_emit::{EmitOpts, SwKind};
use volt_syntax::ParseResult;

#[derive(Parser)]
#[command(name = "volt", version = volt_sv_emit::VOLT_VERSION_TEXT)]
#[command(about = "Volt HDL — clock-domain-safe hardware description language")]
#[command(after_help = "EXAMPLES:
    volt build counter.volt
    volt run counter.volt --vcd waves.vcd
    volt verify counter.volt --mode prove
    volt explain E3001")]
struct Cli {
    /// Diagnostic language: en | tr (priority: flag > VOLT_LANG > Volt.toml [ui] lang > en)
    #[arg(long, global = true, value_enum)]
    lang: Option<LangArg>,
    /// Stop reporting after this many diagnostics; 0 = unlimited (ADR-0068)
    #[arg(long, global = true, default_value_t = DEFAULT_MAX_DIAGNOSTICS)]
    max_diagnostics: usize,
    /// Color output: auto | always | never (default: VOLT_COLOR or auto)
    #[arg(long, global = true, value_enum)]
    color: Option<color::ColorArg>,
    /// Shortcut for --color=never
    #[arg(long, global = true, conflicts_with = "color")]
    no_color: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

/// Tanı üst sınırı varsayılanı (ADR-0068 §4 — Clang `-ferror-limit`
/// emsali, katlama sonrası yalnız FARKLI tanılar sayılır).
const DEFAULT_MAX_DIAGNOSTICS: usize = 1000;
static MAX_DIAGNOSTICS: AtomicUsize = AtomicUsize::new(DEFAULT_MAX_DIAGNOSTICS);

/// cli-contract.md §3 --lang değerleri.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LangArg {
    En,
    Tr,
}

/// Dil önceliği: --lang > VOLT_LANG > Volt.toml [ui] lang > En.
/// Sistem locale'i BİLEREK okunmaz (CI'da sürpriz üretir).
fn resolve_lang(flag: Option<LangArg>) -> Lang {
    match flag {
        Some(LangArg::En) => return Lang::En,
        Some(LangArg::Tr) => return Lang::Tr,
        None => {}
    }
    if let Ok(v) = std::env::var("VOLT_LANG") {
        if let Some(l) = Lang::parse(&v) {
            return l;
        }
    }
    manifest_lang(Path::new("Volt.toml")).unwrap_or(Lang::En)
}

/// Volt.toml `[ui] lang` değerini okur. Tek anahtar için tam TOML
/// ayrıştırıcı bağımlılığı almamak adına bilinçli olarak dar tutuldu.
fn manifest_lang(path: &Path) -> Option<Lang> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_ui = false;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_ui = line == "[ui]";
            continue;
        }
        if !in_ui {
            continue;
        }
        if let Some(rest) = line.strip_prefix("lang") {
            let value = rest.trim_start().strip_prefix('=')?.trim();
            return Lang::parse(value.trim_matches('"'));
        }
    }
    None
}

/// cli-contract.md §3 --format değerleri.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
    Short,
}

#[derive(Subcommand)]
enum Command {
    /// Compile and emit SystemVerilog
    #[command(after_help = "EXAMPLES:
    volt build counter.volt
    volt build --emit=sva design.volt
    volt build --sva inline --emit=sva design.volt
    volt build --emit=sdc,xdc design.volt
    volt build --emit=sdc --sdc-style=clock-groups design.volt
    volt build --emit=c,rust,regmap --check-regmap design.volt
    volt build --format json --target-dir out design.volt
    volt build                      # in a project: its top module(s) (Volt.toml)")]
    Build {
        /// Input .volt file (default: the project's top module, see Volt.toml; ADR-0095)
        file: Option<PathBuf>,
        /// Output directory (default: build/)
        #[arg(long, default_value = "build")]
        target_dir: PathBuf,
        /// Output format: human | json | short
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
        /// Additional outputs: sva (assertions), rust | c (drivers, build/sw/),
        /// regmap (build/sw/<module>.json), regmap-md (build/docs/<module>.md),
        /// sdc | xdc (timing constraints, build/constraints/<module>.sdc)
        #[arg(long, value_enum, value_delimiter = ',')]
        emit: Vec<EmitArg>,
        /// SVA placement: separate .sva file with bind | inline in the .sv
        #[arg(long, value_enum, default_value_t = SvaArg::Separate)]
        sva: SvaArg,
        /// Write all modules into one build/rtl/<source>.sv (pre-ADR-0024 layout)
        #[arg(long)]
        single_file: bool,
        /// Compare the generated drivers with the generated RTL address decode (ADR-0063)
        #[arg(long)]
        check_regmap: bool,
        /// Constraints between clock domains (--emit=sdc|xdc): targeted (per
        /// synchronizer; other crossings stay timed) | clock-groups (ADR-0054)
        #[arg(long, value_enum, default_value_t = SdcStyleArg::Targeted)]
        sdc_style: SdcStyleArg,
    },
    /// Compare a generated driver (.h, .rs, regmap .json) with the design's register map (ADR-0063)
    #[command(after_help = "EXAMPLES:
    volt check-regmap gpio.volt --against firmware/gpio.h
    volt check-regmap gpio.volt --against gpio.h --against gpio.rs
    volt check-regmap --format json gpio.volt --against build/sw/gpio.json")]
    CheckRegmap {
        /// Input .volt file (the register map's source of truth)
        file: PathBuf,
        /// A file generated earlier by volt build --emit=c|rust|regmap (repeatable)
        #[arg(long, required = true)]
        against: Vec<PathBuf>,
        /// Output format: human | json | short
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Fast check (produces no output files)
    #[command(after_help = "EXAMPLES:
    volt check design.volt
    volt check --format short design.volt
    volt check                      # in a project: every source file")]
    Check {
        /// Input .volt file (default: every source file of the project; ADR-0095)
        file: Option<PathBuf>,
        /// Output format: human | json | short
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Formally verify contracts with SymbiYosys (exit 6 counterexample, 7 unknown, 8 timeout)
    #[command(after_help = "EXAMPLES:
    volt verify design.volt
    volt verify --mode prove design.volt
    volt verify --depth 40 --engine bitwuzla design.volt
    volt verify -j 8 examples/soc/top.volt
    volt verify -j 1 --fail-fast design.volt
    volt verify                     # in a project: its top module (Volt.toml)")]
    Verify {
        /// Input .volt file (default: the project's top module, see Volt.toml; ADR-0095)
        file: Option<PathBuf>,
        /// Search depth in cycles (BMC bound / induction length)
        #[arg(long, default_value_t = 20)]
        depth: u32,
        /// Parallel sby jobs: a number or 'auto' (= CPU count); 1 runs modules sequentially
        #[arg(short = 'j', long, default_value = "auto", value_parser = verify_jobs::parse_jobs)]
        jobs: verify_jobs::Jobs,
        /// Stop at the first counterexample (default: every module task completes)
        #[arg(long)]
        fail_fast: bool,
        /// SMT solver: boolector | bitwuzla | yices | z3 (ADR-0082)
        #[arg(long, value_enum, default_value_t = EngineArg::Boolector)]
        engine: EngineArg,
        /// Verification mode: bmc | prove | cover
        #[arg(long, value_enum, default_value_t = VerifyModeArg::Bmc)]
        mode: VerifyModeArg,
        /// Time limit per module task in seconds (exit 8 when reached; default: none)
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
        timeout: Option<u32>,
        /// Output directory (default: build/)
        #[arg(long, default_value = "build")]
        target_dir: PathBuf,
        /// Output format: human | json | short
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Compile and simulate with Verilator (ADR-0033; cli-contract.md §7)
    #[command(after_help = "EXAMPLES:
    volt run design.volt
    volt run --cycles 500 design.volt
    volt run --vcd waves.vcd design.volt
    volt run --contracts design.volt
    volt run                        # in a project: its top module (Volt.toml)")]
    Run {
        /// Input .volt file (default: the project's top module, see Volt.toml; ADR-0095)
        file: Option<PathBuf>,
        /// Number of clock cycles to simulate
        #[arg(long, default_value_t = 100)]
        cycles: u64,
        /// Write a VCD waveform to this file
        #[arg(long)]
        vcd: Option<PathBuf>,
        /// Top module (default: the only module in the file; in a project, Volt.toml `top`)
        #[arg(long)]
        top: Option<String>,
        /// Also run the design's contracts as simulation monitors (ADR-0064)
        #[arg(long)]
        contracts: bool,
        /// Output directory (default: build/)
        #[arg(long, default_value = "build")]
        target_dir: PathBuf,
    },
    /// Run simulation tests with Verilator (ADR-0033; cli-contract.md §8)
    #[command(after_help = "EXAMPLES:
    volt test
    volt test my_design_test.volt
    volt test uart --nocapture
    volt test --no-contracts
    volt test --waves               # record every test's waveform
    volt test --watch               # re-run when a project file changes")]
    Test {
        /// A .volt test file, or a substring filter over test names
        filter: Option<String>,
        /// Also stream the raw testbench output
        #[arg(long)]
        nocapture: bool,
        /// Record the waveform of every test (default: only failed tests, re-run with tracing; ADR-0095)
        #[arg(long, conflicts_with = "no_waves")]
        waves: bool,
        /// Record no waveforms
        #[arg(long)]
        no_waves: bool,
        /// Re-run the tests whenever a project file changes (Ctrl-C stops)
        #[arg(long)]
        watch: bool,
        /// Do not run contracts as simulation monitors (ADR-0064; on by default)
        #[arg(long)]
        no_contracts: bool,
        /// Output directory (default: build/)
        #[arg(long, default_value = "build")]
        target_dir: PathBuf,
    },
    /// Report which commands work on this machine and what to install (ADR-0084)
    #[command(after_help = "EXAMPLES:
    volt doctor
    volt doctor --format json
    volt doctor --strict            # exit 3 if test, run or verify lacks a tool
    volt --lang tr doctor")]
    Doctor {
        /// Output format: human | json
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
        /// Exit 3 when a required capability (test/run, verify) is not fully available
        #[arg(long)]
        strict: bool,
        /// Time limit per tool version query, in seconds
        #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u64).range(1..=60))]
        timeout: u64,
    },
    /// Create a new project in a new directory (ADR-0084)
    #[command(after_help = "EXAMPLES:
    volt new blinky
    volt new my_fifo --template fifo
    volt new --list")]
    New {
        /// Project name (a Volt identifier); also the directory name
        #[arg(required_unless_present = "list")]
        name: Option<String>,
        /// Template: minimal | cdc | fifo | mmio
        #[arg(long, default_value = new::templates::DEFAULT)]
        template: String,
        /// List the templates
        #[arg(long)]
        list: bool,
    },
    /// Create a project in the current directory; never overwrites files (ADR-0084)
    #[command(after_help = "EXAMPLES:
    volt init
    volt init --template cdc --name my_bridge")]
    Init {
        /// Template: minimal | cdc | fifo | mmio
        #[arg(long, default_value = new::templates::DEFAULT)]
        template: String,
        /// Project name (default: the directory name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Start the Volt language server on stdio (editors connect here)
    Lsp,
    /// Explain a diagnostic code or topic in detail (cli-contract.md §9)
    #[command(after_help = "EXAMPLES:
    volt explain E3001
    volt explain domains
    volt explain --topics
    volt explain --list")]
    Explain {
        /// Diagnostic code, e.g. E3001 (case-insensitive), or a topic name
        #[arg(required_unless_present_any = ["list", "topics"])]
        code: Option<String>,
        /// List all codes grouped by category
        #[arg(long)]
        list: bool,
        /// List all topics ('volt explain <topic>')
        #[arg(long)]
        topics: bool,
    },
}

/// `--emit` ek çıktıları (F4a `sva`; ADR-0053 yazılım tarafı; ADR-0054
/// zamanlama kısıtları).
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum EmitArg {
    Sva,
    Rust,
    C,
    Regmap,
    RegmapMd,
    Sdc,
    Xdc,
}

impl EmitArg {
    /// Yazılım çıktısı türü; `sva`/`sdc`/`xdc` için None.
    fn sw_kind(self) -> Option<SwKind> {
        match self {
            EmitArg::Sva | EmitArg::Sdc | EmitArg::Xdc => None,
            EmitArg::Rust => Some(SwKind::Rust),
            EmitArg::C => Some(SwKind::C),
            EmitArg::Regmap => Some(SwKind::Json),
            EmitArg::RegmapMd => Some(SwKind::Markdown),
        }
    }

    /// Kısıt lehçesi; diğer türler için None.
    fn dialect(self) -> Option<Dialect> {
        match self {
            EmitArg::Sdc => Some(Dialect::Sdc),
            EmitArg::Xdc => Some(Dialect::Xdc),
            _ => None,
        }
    }
}

/// `--sva` yerleşimi (F4a ADIM 3); varsayılan ayrı dosya + bind.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SvaArg {
    Separate,
    Inline,
}

/// `--sdc-style` (ADR-0065 §4.3); varsayılan hedefli kısıtlar.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SdcStyleArg {
    Targeted,
    ClockGroups,
}

impl SdcStyleArg {
    fn style(self) -> SdcStyle {
        match self {
            SdcStyleArg::Targeted => SdcStyle::Targeted,
            SdcStyleArg::ClockGroups => SdcStyle::ClockGroups,
        }
    }
}

/// `volt verify --engine` (F4b) — sby'ye geçen SMT çözücüsü.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum EngineArg {
    Boolector,
    Bitwuzla,
    Yices,
    Z3,
}

impl From<EngineArg> for SbyEngine {
    fn from(a: EngineArg) -> Self {
        match a {
            EngineArg::Z3 => SbyEngine::Z3,
            EngineArg::Boolector => SbyEngine::Boolector,
            EngineArg::Yices => SbyEngine::Yices,
            EngineArg::Bitwuzla => SbyEngine::Bitwuzla,
        }
    }
}

/// `volt verify --mode` (F4b) — sby doğrulama kipi.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum VerifyModeArg {
    Bmc,
    Prove,
    Cover,
}

impl From<VerifyModeArg> for SbyMode {
    fn from(a: VerifyModeArg) -> Self {
        match a {
            VerifyModeArg::Bmc => SbyMode::Bmc,
            VerifyModeArg::Prove => SbyMode::Prove,
            VerifyModeArg::Cover => SbyMode::Cover,
        }
    }
}

fn main() -> ExitCode {
    // Tüm komutlar derleyici yığınında (ADR-0080): derinlik sınırındaki
    // bir ağacı her geçit, her platformda yığını taşırmadan yürür
    // (Windows ana iş parçacığı yalnız 1 MB).
    volt_syntax::with_compiler_stack(run)
}

fn run() -> ExitCode {
    let cli = Cli::parse();
    volt_diagnostics::set_lang(resolve_lang(cli.lang));
    MAX_DIAGNOSTICS.store(cli.max_diagnostics, Ordering::Relaxed);
    // cli-contract.md §3: tanılar stderr'e yazar, renk kararı ona göre.
    let color = color::flag(cli.color, cli.no_color);
    let color_env = color::ColorEnv::from_process();
    let stderr = std::io::IsTerminal::is_terminal(&std::io::stderr());
    volt_diagnostics::set_color(color::enabled(
        color,
        &color_env,
        stderr && color::terminal_takes_ansi(),
    ));
    // Komutsuz çağrı hata DEĞİL, yol göstermedir (UX Anayasası:
    // en iyi onboarding olmayan onboarding) — sık görevler + çıkış 0.
    let Some(command) = cli.command else {
        print_no_command_help();
        return ExitCode::SUCCESS;
    };
    match command {
        Command::Build {
            file,
            target_dir,
            format,
            emit,
            sva,
            single_file,
            check_regmap,
            sdc_style,
        } => {
            let mode = if emit.contains(&EmitArg::Sva) {
                match sva {
                    SvaArg::Separate => SvaMode::Separate,
                    SvaArg::Inline => SvaMode::Inline,
                }
            } else {
                SvaMode::None
            };
            // Aynı tür iki kez yazıldıysa (rust,rust) bir kez üretilir.
            let mut sw: Vec<SwKind> = Vec::new();
            for kind in emit.iter().filter_map(|e| e.sw_kind()) {
                if !sw.contains(&kind) {
                    sw.push(kind);
                }
            }
            let mut dialects: Vec<Dialect> = Vec::new();
            for d in emit.iter().filter_map(|e| e.dialect()) {
                if !dialects.contains(&d) {
                    dialects.push(d);
                }
            }
            let files = match file {
                Some(f) => vec![f],
                None => match project::load("build").and_then(|p| project::tops(&p, "build")) {
                    Ok(tops) => project::top_files(&tops),
                    Err(code) => return code,
                },
            };
            // Proje kipinde birden çok üst modül dosyası: sırayla, en
            // kötü çıkış kodu (ADR-0095).
            let mut worst = ExitCode::SUCCESS;
            for f in &files {
                let code = build(
                    f,
                    &target_dir,
                    format,
                    mode,
                    single_file,
                    &SwRequest {
                        kinds: &sw,
                        check_regmap,
                    },
                    &SdcRequest {
                        dialects: &dialects,
                        style: sdc_style.style(),
                    },
                );
                if code != ExitCode::SUCCESS {
                    worst = code;
                }
            }
            worst
        }
        Command::Check { file, format } => match file {
            Some(f) => check(&f, format),
            None => check_project(format),
        },
        Command::CheckRegmap {
            file,
            against,
            format,
        } => regmap_check::check_regmap(&file, &against, format),
        Command::Verify {
            file,
            depth,
            jobs,
            fail_fast,
            engine,
            mode,
            timeout,
            target_dir,
            format,
        } => {
            let file = match file {
                Some(f) => f,
                None => match project::load("verify")
                    .and_then(|p| project::single_top(&p, "verify", None))
                {
                    Ok(top) => top.file,
                    Err(code) => return code,
                },
            };
            verify::verify(
                &file,
                &target_dir,
                format,
                SbyOptions {
                    mode: mode.into(),
                    depth,
                    engine: engine.into(),
                    // Görev başına verify.rs'te ayarlanır (multiclock_modules).
                    multiclock: false,
                    timeout,
                },
                verify::VerifyArgs { jobs, fail_fast },
            )
        }
        Command::Run {
            file,
            cycles,
            vcd,
            top,
            contracts,
            target_dir,
        } => {
            // Proje kipi: dosya ve üst modül Volt.toml'dan (ADR-0095).
            let (file, top) = match file {
                Some(f) => (f, top),
                None => match project::load("run")
                    .and_then(|p| project::single_top(&p, "run", top.as_deref()))
                {
                    Ok(t) => (t.file, Some(t.module)),
                    Err(code) => return code,
                },
            };
            sim::run(
                &file,
                sim::RunOptions {
                    cycles,
                    vcd: vcd.as_deref(),
                    top: top.as_deref(),
                    contracts,
                    target_dir: &target_dir,
                },
            )
        }
        Command::Test {
            filter,
            nocapture,
            waves,
            no_waves,
            watch,
            no_contracts,
            target_dir,
        } => {
            let opts = sim::TestOptions {
                nocapture,
                contracts: !no_contracts,
                waves: match (waves, no_waves) {
                    (true, _) => sim::WaveMode::All,
                    (_, true) => sim::WaveMode::Off,
                    _ => sim::WaveMode::OnFailure,
                },
                target_dir: &target_dir,
            };
            if watch {
                watch::watch(filter.as_deref(), opts)
            } else {
                sim::test(filter.as_deref(), opts)
            }
        }
        Command::Doctor {
            format,
            strict,
            timeout,
        } => doctor::doctor(doctor::DoctorOptions {
            format,
            strict,
            timeout: std::time::Duration::from_secs(timeout),
        }),
        Command::New {
            name,
            template,
            list,
        } => match name {
            Some(name) if !list => new::new(
                &name,
                new::NewOptions {
                    template: &template,
                },
            ),
            _ => new::list(),
        },
        Command::Init { template, name } => new::init(
            name.as_deref(),
            new::NewOptions {
                template: &template,
            },
        ),
        Command::Lsp => {
            volt_lsp::run_stdio();
            ExitCode::SUCCESS
        }
        Command::Explain { code, list, topics } => {
            // `volt explain` stdout'a yazar: renk kararı ona göre.
            let stdout = std::io::IsTerminal::is_terminal(&std::io::stdout());
            let color = color::enabled(color, &color_env, stdout && color::terminal_takes_ansi());
            explain_cmd(code.as_deref(), list, topics, color)
        }
    }
}

/// `volt` (argümansız): sürüm + sık görevler, stdout'a, çıkış 0.
fn print_no_command_help() {
    let version = volt_sv_emit::VOLT_VERSION_TEXT;
    print!(
        "{}",
        lstr!(
            en: "Volt HDL {version}\n\n\
                 No command given. Common tasks:\n\
                 \x20   volt build design.volt      Compile to SystemVerilog\n\
                 \x20   volt check design.volt      Check without producing output\n\
                 \x20   volt run design.volt        Simulate\n\
                 \x20   volt test                   Run tests\n\
                 \x20   volt verify design.volt     Prove contracts\n\
                 \x20   volt explain E3001          Explain an error code\n\n\
                 Run 'volt --help' for all commands.\n";
            tr: "Volt HDL {version}\n\n\
                 Komut verilmedi. Sık görevler:\n\
                 \x20   volt build tasarim.volt     SystemVerilog'a derle\n\
                 \x20   volt check tasarim.volt     Çıktı üretmeden denetle\n\
                 \x20   volt run tasarim.volt       Simüle et\n\
                 \x20   volt test                   Testleri koştur\n\
                 \x20   volt verify tasarim.volt    Kontratları kanıtla\n\
                 \x20   volt explain E3001          Bir hata kodunu açıkla\n\n\
                 Tüm komutlar için 'volt --help' çalıştırın.\n"
        )
    );
}

/// `volt explain` — açıklama metni stdout verisidir (§11), tanılar ve
/// kullanım hataları stderr'e gider. Bilinmeyen kod: çıkış kodu 2.
fn explain_cmd(code: Option<&str>, list: bool, topics: bool, color: bool) -> ExitCode {
    let lang = volt_diagnostics::lang();
    if list {
        print!("{}", explain::render_list(lang));
        return ExitCode::SUCCESS;
    }
    if topics {
        print!("{}", explain::topics::render_topic_list(lang));
        return ExitCode::SUCCESS;
    }
    let input = code.expect("clap: code, --list veya --topics zorunlu");
    let Some(parsed) = ErrorCode::parse(input) else {
        // F4b: kod değilse konu dene ('volt explain verify-setup').
        if let Some(page) = explain::topics::render_topic(input, lang, terminal_width(), color) {
            print!("{page}");
            return ExitCode::SUCCESS;
        }
        eprintln!(
            "{}",
            lstr!(
                en: "error: unknown code '{}'", input;
                tr: "hata: bilinmeyen kod '{}'", input
            )
        );
        if let Some(similar) = explain::suggest(input) {
            eprintln!(
                "{}",
                lstr!(
                    en: "did you mean '{}'?", similar.as_str();
                    tr: "şunu mu demek istediniz: '{}'?", similar.as_str()
                )
            );
        }
        return ExitCode::from(2);
    };
    print!(
        "{}",
        explain::render_explanation(parsed, lang, terminal_width(), color)
    );
    ExitCode::SUCCESS
}

/// Sarma genişliği: COLUMNS > 80 varsayılanı (§9). Alt sınırı
/// render_explanation uygular.
fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(explain::DEFAULT_WIDTH)
}

struct Compiled {
    map: SourceMap,
    diagnostics: Vec<Diagnostic>,
    /// Ayrıştırılan AST — `run`/`test` port bilgisi için (ADR-0033).
    ast: volt_ast::SourceFile,
    /// Yalnız tüm aşamalar hatasızsa üretilir.
    sv: Option<String>,
    /// Modül başına SV (ADR-0024) — `sv` ile aynı koşulda dolu.
    modules: Vec<SvModule>,
    /// Birimdeki dosya sayısı (ana dosya dahil; ADR-0042 ölçümü).
    file_count: usize,
    /// `use` ile yüklenen dosyalar (ana dosya hariç) — çıktı kümesi
    /// yalnız ana dosyanın modüllerinden başlar (`reach.rs`).
    library_files: Vec<FileId>,
    /// Birleşik SV başlığındaki kaynak adı (ana dosyanın adı).
    source_name: String,
    /// `--emit=sva` ayrı modunda kontratlı modüllerin .sva içerikleri.
    sva_files: Vec<SvaFile>,
    /// Üretilen property kimlikleri (F4b `verify` — sby FAIL eşlemesi).
    sva_props: Vec<SvaProp>,
    /// Kontratı olup saat portu olmayan modüller (ADR-0097): `verify`
    /// erişilebilir olanlar için E5005 üretir.
    unclocked_contracts: Vec<volt_sv_emit::UnclockedContracts>,
    /// İki+ saat portlu modüller — `.sby`'ye `multiclock on` (ADR-0027).
    multiclock_modules: Vec<String>,
    /// `@mmio` modüllerinin register haritaları (ADR-0053) — `--emit=rust,
    /// c,regmap,regmap-md` bunlardan üretilir; hata varsa boş.
    regmaps: Vec<volt_ast::mmio::RegMap>,
    /// Zamanlama kısıtı modeli (ADR-0054) — `--emit=sdc,xdc` bundan
    /// üretilir; anlamsal aşama hatalıysa boş.
    constraints: volt_hir::ConstraintResult,
    /// `@source`'lu extern modüllerin çözülmüş SV dosyaları (ADR-0076) —
    /// `run`/`test`/`verify` bunları araca üretilen SV ile birlikte verir.
    extern_sources: Vec<volt_hir::ExternSourceFile>,
    /// Enum/Trit sinyalleri — dalga formu oturumu (ADR-0092); `sv` ile
    /// aynı koşulda dolu.
    waves: volt_sv_emit::WaveInfo,
}

impl Compiled {
    fn errors(&self) -> usize {
        count_errors(&self.diagnostics)
    }

    fn warnings(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }
}

fn count_errors(diags: &[Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count()
}

/// Ortak boru hattı — aşamalar sırayla, hatalı aşamadan sonrakiler
/// atlanır: parse (E0xxx) → resolve (E1xxx) → const+typeck
/// (E2xxx/E4xxx) → domain (E3xxx) → emit (yalnız `want_sv`).
///
/// `check` emit geçişini de koşar ama çıktıyı atar (§6 "çıktı üretmeden
/// doğrulama"; ADR-0070): SV eşlemesi henüz olmayan yapılar (E0003) ve
/// emitter'ın diğer denetimleri `check`'te ve editörde de görünür. Emit
/// yalnız önceki aşamalar hatasızsa koşar — analizi engellemez.
fn compile(file: &Path, want_sv: bool, sva_mode: SvaMode) -> Result<Compiled, ExitCode> {
    let mut compiled = compile_all(file, want_sv, sva_mode)?;
    cap_diagnostics(&mut compiled.diagnostics);
    Ok(compiled)
}

/// Tanı üst sınırı (ADR-0068 §4, W0023) — ortak uygulama
/// `volt_diagnostics::cap_diagnostics` (LSP de kullanır, ADR-0070).
fn cap_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    volt_diagnostics::cap_diagnostics(
        diagnostics,
        MAX_DIAGNOSTICS.load(Ordering::Relaxed),
        &lstr!(
            en: "fix the reported diagnostics first, or raise the limit with --max-diagnostics=N (0 = unlimited)";
            tr: "önce raporlanan tanıları düzeltin ya da sınırı --max-diagnostics=N ile yükseltin (0 = sınırsız)"
        ),
    );
}

fn compile_all(file: &Path, want_sv: bool, sva_mode: SvaMode) -> Result<Compiled, ExitCode> {
    // ── Aşama 0+1: dosya keşfi (ADR-0042) + birim ayrıştırma ──
    let unit = match unit::load_unit(file) {
        Ok(u) => u,
        Err(err) => {
            eprintln!(
                "{}",
                lstr!(
                    en: "error: cannot read '{}': {}", file.display(), err;
                    tr: "hata: '{}' okunamadı: {}", file.display(), err
                )
            );
            return Err(ExitCode::from(3));
        }
    };
    let names = unit.source_names();
    let map = unit.map;
    let parsed = unit.parsed;
    let file_count = unit.files.len();
    // Ana dosya ilk ziyaret edilir: FileId(0) (unit_load.rs).
    let library_files: Vec<FileId> = unit
        .files
        .iter()
        .map(|(fid, _)| *fid)
        .filter(|fid| *fid != FileId(0))
        .collect();
    let source_name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.display().to_string());
    let lint_unenforced = unit
        .manifest
        .as_ref()
        .map_or_else(Default::default, |m| m.lint_unenforced);
    let mut diagnostics = parsed.diagnostics.clone();
    let fail = |map: SourceMap, diagnostics: Vec<Diagnostic>, ast: volt_ast::SourceFile| Compiled {
        map,
        // Açılmış `for` yinelemelerine düşen tanılara bağlam notu (ADR-0056).
        diagnostics: volt_hir::annotate_generate(&ast, diagnostics),
        ast,
        sv: None,
        modules: Vec::new(),
        file_count,
        library_files: library_files.clone(),
        source_name: source_name.clone(),
        sva_files: Vec::new(),
        sva_props: Vec::new(),
        unclocked_contracts: Vec::new(),
        multiclock_modules: Vec::new(),
        regmaps: Vec::new(),
        constraints: Default::default(),
        extern_sources: Vec::new(),
        waves: Default::default(),
    };
    if count_errors(&diagnostics) > 0 {
        return Ok(fail(map, diagnostics, parsed.ast));
    }

    // ── Aşama 1b: uygulanmayan nitelikler (ADR-0048, W0021) — Volt.toml
    // `[lint] unenforced_attributes = "allow"` ile susturulabilir.
    diagnostics.extend(volt_hir::pre_resolve_checks(&parsed.ast, lint_unenforced));

    // ── Aşama 2a: import çözümlemesi — bulunamayan dosya (E1011),
    // döngü (E1006), özel öğe (E1004), belirsizlik (E1010) ──
    diagnostics.extend(unit.diagnostics);
    let imports = volt_hir::check_imports(&parsed.ast, &unit.info);
    diagnostics.extend(imports.diagnostics);
    // ── Aşama 2b: extern SV kaynakları (ADR-0076) — `@source` yolu proje
    // dışında ya da dosya yok: E1012 ──
    let locator = volt_hir::FsSourceLocator::new(&unit.files);
    let (extern_sources, extern_diags) = volt_hir::resolve_extern_sources(&parsed.ast, &locator);
    diagnostics.extend(extern_diags);
    if count_errors(&diagnostics) > 0 {
        return Ok(fail(map, diagnostics, parsed.ast));
    }

    // Test veri dosyaları (ADR-0058) ana dosyaya göre çözülür.
    let test_files = sim_lower::FsTestFiles::for_test_file(file);
    // `X_test.volt`: test blokları `volt test` ile aynı tam denetimden
    // geçer (kardeş tasarım varsa onunla, ADR-0101).
    let test_target = volt_hir::unit_load::TestTarget::for_unit(file);
    let Some(constraints) = run_semantic_stages(
        &parsed,
        &imports.scopes,
        &test_files,
        &test_target,
        &mut diagnostics,
    ) else {
        return Ok(fail(map, diagnostics, parsed.ast));
    };
    if count_errors(&diagnostics) > 0 {
        return Ok(fail(map, diagnostics, parsed.ast));
    }

    // ── Aşama 5: emit (E2005 literal boyutlandırma; F4a SVA). `check`
    // için de koşar, çıktısı atılır (ADR-0070) ──
    let sources = volt_sv_emit::unit_source_texts(&names, &map);
    let emitted = volt_sv_emit::emit_unit(
        &parsed.ast,
        &source_name,
        &sources,
        sva_mode,
        ConstArrayStyle::default(),
    );
    diagnostics.extend(emitted.diagnostics);
    // Açılmış `for` yinelemelerine düşen tanılara bağlam notu (ADR-0056).
    let diagnostics = volt_hir::annotate_generate(&parsed.ast, diagnostics);
    let (sv, modules, sva_files, sva_props, multiclock_modules, waves) =
        if want_sv && count_errors(&diagnostics) == 0 {
            (
                Some(emitted.sv),
                emitted.modules,
                emitted.sva_files,
                emitted.sva_props,
                emitted.multiclock_modules,
                emitted.waves,
            )
        } else {
            Default::default()
        };
    let unclocked_contracts = emitted.unclocked_contracts;

    Ok(Compiled {
        map,
        diagnostics,
        ast: parsed.ast,
        sv,
        modules,
        file_count,
        library_files,
        source_name,
        sva_files,
        sva_props,
        unclocked_contracts,
        multiclock_modules,
        regmaps: parsed.regmaps,
        constraints,
        extern_sources,
        waves,
    })
}

/// Aşama 2-4: birim modu çözümleme (ADR-0042) + paylaşılan kapılı
/// boru hattı (ADR-0070, LSP ile aynı fonksiyon). Tanılar `out`'a
/// eklenir; bir aşama hata üretirse `None`. Başarıda zamanlama kısıtı
/// modeli (ADR-0054) döner.
fn run_semantic_stages(
    parsed: &ParseResult,
    scopes: &std::collections::HashMap<FileId, FileScope>,
    test_files: &dyn volt_hir::TestFileLoader,
    tests: &volt_hir::unit_load::TestTarget,
    out: &mut Vec<Diagnostic>,
) -> Option<volt_hir::ConstraintResult> {
    let resolve = volt_hir::resolve_unit(&parsed.ast, scopes);
    volt_hir::run_semantic_stages(&parsed.ast, resolve, Some(test_files), tests, out).constraints
}

/// Tanıları seçilen formatta stderr'e yazar (JSON zarfı hariç — o
/// stdout verisidir, §11).
fn render_diagnostics(compiled: &Compiled, format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            for diag in &compiled.diagnostics {
                eprintln!("{}", render_human(diag, &compiled.map));
            }
        }
        OutputFormat::Short => {
            for diag in &compiled.diagnostics {
                eprintln!("{}", render_short(diag, &compiled.map));
            }
        }
        OutputFormat::Json => {}
    }
}

/// cli-contract.md §5 JSON zarfı — stdout'a tek belge.
fn print_json_envelope(command: &str, compiled: &Compiled, artifacts: &[String], started: Instant) {
    let envelope = json_envelope(command, compiled, artifacts, started);
    println!(
        "{}",
        serde_json::to_string_pretty(&envelope).expect("JSON zarfı")
    );
}

/// JSON zarfının kendisi (cli-contract.md §5); `verify` kendi nesnesini
/// ekleyip basar (ADR-0055).
fn json_envelope(
    command: &str,
    compiled: &Compiled,
    artifacts: &[String],
    started: Instant,
) -> serde_json::Value {
    // CI `diagnostics[0]`'da engelleyici hatayı bekler: hatalar önce,
    // uyarılar sonra (kendi içlerinde kaynak sırası korunur).
    let mut ordered: Vec<&Diagnostic> = compiled.diagnostics.iter().collect();
    ordered.sort_by_key(|d| d.severity != Severity::Error);
    serde_json::json!({
        "version": "1",
        "command": command,
        "success": compiled.errors() == 0,
        "diagnostics": ordered
            .iter()
            .map(|d| to_json_value(d, &compiled.map))
            .collect::<Vec<_>>(),
        "summary": { "errors": compiled.errors(), "warnings": compiled.warnings() },
        "artifacts": artifacts,
        "duration_ms": started.elapsed().as_millis() as u64,
    })
}

/// `volt build` yazılım tarafı isteği (ADR-0053 `--emit`, ADR-0063
/// `--check-regmap`).
struct SwRequest<'a> {
    kinds: &'a [SwKind],
    check_regmap: bool,
}

/// `volt build` kısıt isteği (ADR-0054 `--emit=sdc,xdc`, ADR-0065
/// `--sdc-style`).
struct SdcRequest<'a> {
    dialects: &'a [Dialect],
    style: SdcStyle,
}

fn build(
    file: &Path,
    target_dir: &Path,
    format: OutputFormat,
    sva_mode: SvaMode,
    single_file: bool,
    sw: &SwRequest<'_>,
    sdc: &SdcRequest<'_>,
) -> ExitCode {
    let dialects = sdc.dialects;
    let start = Instant::now();
    if format == OutputFormat::Human {
        eprintln!(
            "{}",
            lstr!(
                en: "   Compiling {}", file.display();
                tr: "   Derleniyor {}", file.display()
            )
        );
    }

    let mut compiled = match compile(file, true, sva_mode) {
        Ok(c) => c,
        Err(code) => return code,
    };
    // ADR-0042 ek: çıktı = ana dosyanın modülleri + örnekleme kapanışı.
    compiled.retain_reachable();
    // ADR-0054: W0022 yalnız kısıt dosyası istendiğinde — SDC istemeyen
    // bir tasarımdan frekans istenmez.
    if !dialects.is_empty() {
        let warnings = compiled.constraints.missing_frequency_warnings();
        compiled.diagnostics.extend(warnings);
    }
    render_diagnostics(&compiled, format);

    // Hata varsa SV ÜRETİLMEZ — hatalı tasarım sentezlenemez.
    let Some(sv) = &compiled.sv else {
        if format == OutputFormat::Human {
            eprintln!(
                "{}",
                lstr!(
                    en: "     Error: build failed due to {} error(s), {} warning(s)",
                        compiled.errors(), compiled.warnings();
                    tr: "     Hata: {} hata, {} uyarı nedeniyle derleme başarısız",
                        compiled.errors(), compiled.warnings()
                )
            );
        }
        if format == OutputFormat::Json {
            print_json_envelope("build", &compiled, &[], start);
        }
        return ExitCode::from(1);
    };

    let rtl_dir = target_dir.join("rtl");
    if let Err(err) = std::fs::create_dir_all(&rtl_dir) {
        eprintln!(
            "{}",
            lstr!(
                en: "error: cannot create '{}': {}", rtl_dir.display(), err;
                tr: "hata: '{}' oluşturulamadı: {}", rtl_dir.display(), err
            )
        );
        return ExitCode::from(3);
    }
    // ADR-0024: modül başına bir dosya (build/rtl/<Modül>.sv);
    // `--single-file` eski düzeni (build/rtl/<kaynak>.sv) korur. Modülsüz
    // birim (yalnız fn/const/tip — kütüphane) SV üretmez (ADR-0042 ek):
    // modülsüz .sv'yi Verilator `--top-module` ile reddeder.
    let outputs: Vec<(PathBuf, &str)> = if compiled.modules.is_empty() {
        Vec::new()
    } else if single_file {
        let stem = file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        vec![(rtl_dir.join(format!("{stem}.sv")), sv.as_str())]
    } else {
        compiled
            .modules
            .iter()
            .map(|m| (rtl_dir.join(format!("{}.sv", m.name)), m.sv.as_str()))
            .collect()
    };
    for (path, content) in &outputs {
        if let Err(err) = std::fs::write(path, content) {
            eprintln!(
                "{}",
                lstr!(
                    en: "error: cannot write '{}': {}", path.display(), err;
                    tr: "hata: '{}' yazılamadı: {}", path.display(), err
                )
            );
            return ExitCode::from(3);
        }
    }
    let sv_count = outputs.len();

    // F4a — ayrı SVA dosyaları: build/formal/<modul>.sva.
    let mut artifacts: Vec<String> = outputs
        .iter()
        .map(|(p, _)| p.display().to_string())
        .collect();
    if !compiled.sva_files.is_empty() {
        let formal_dir = target_dir.join("formal");
        if let Err(err) = std::fs::create_dir_all(&formal_dir) {
            eprintln!(
                "{}",
                lstr!(
                    en: "error: cannot create '{}': {}", formal_dir.display(), err;
                    tr: "hata: '{}' oluşturulamadı: {}", formal_dir.display(), err
                )
            );
            return ExitCode::from(3);
        }
        for sva in &compiled.sva_files {
            let sva_path = formal_dir.join(format!("{}.sva", sva.module_name.to_lowercase()));
            if let Err(err) = std::fs::write(&sva_path, &sva.content) {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: cannot write '{}': {}", sva_path.display(), err;
                        tr: "hata: '{}' yazılamadı: {}", sva_path.display(), err
                    )
                );
                return ExitCode::from(3);
            }
            artifacts.push(sva_path.display().to_string());
        }
    }

    // ADR-0053 — yazılım tarafı: build/sw/<modül>.{rs,h,json}, build/docs/<modül>.md.
    let sw_opts = EmitOpts {
        source: file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.display().to_string()),
        version: volt_sv_emit::VOLT_VERSION.to_string(),
    };
    if !sw.kinds.is_empty() {
        match write_sw_outputs(target_dir, &compiled.regmaps, sw.kinds, &sw_opts, format) {
            Ok(paths) => artifacts.extend(paths),
            Err(code) => return code,
        }
    }
    // ADR-0063 Seviye 1 — üretilen sürücü ↔ üretilen RTL adres çözümlemesi.
    let mut regmap_checked = 0;
    if sw.check_regmap {
        let diags = regmap_check::build_check(&compiled, sw.kinds, &sw_opts);
        if !diags.is_empty() {
            regmap_check::render(&diags, &compiled.map, format);
            compiled.diagnostics.extend(diags);
            if format == OutputFormat::Human {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "     Error: register map check failed ({} error(s))", compiled.errors();
                        tr: "     Hata: register haritası denetimi başarısız ({} hata)", compiled.errors()
                    )
                );
            }
            if format == OutputFormat::Json {
                print_json_envelope("build", &compiled, &artifacts, start);
            }
            return ExitCode::from(1);
        }
        regmap_checked = compiled.regmaps.len();
    }

    // ADR-0054 — zamanlama kısıtları: build/constraints/<modül>.{sdc,xdc}.
    if !dialects.is_empty() {
        let opts = volt_sdc_emit::EmitOpts {
            source: file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| file.display().to_string()),
            version: volt_sv_emit::VOLT_VERSION_TEXT.to_string(),
            style: sdc.style,
        };
        match write_constraint_outputs(target_dir, &compiled.constraints, dialects, &opts, format) {
            Ok(paths) => artifacts.extend(paths),
            Err(code) => return code,
        }
    }

    if format == OutputFormat::Human {
        eprintln!(
            "{}",
            lstr!(
                en: "    Finished {:.2}s ({} source file(s), {} SV file(s))",
                    start.elapsed().as_secs_f64(), compiled.file_count, sv_count;
                tr: "    Tamamlandı {:.2}s ({} kaynak dosya, {} SV dosyası)",
                    start.elapsed().as_secs_f64(), compiled.file_count, sv_count
            )
        );
        for (path, content) in &outputs {
            eprintln!(
                "{}",
                lstr!(
                    en: "     Output {} ({} lines)", path.display(), content.lines().count();
                    tr: "     Çıktı {} ({} satır)", path.display(), content.lines().count()
                )
            );
        }
        for artifact in artifacts.iter().skip(outputs.len()) {
            eprintln!(
                "{}",
                lstr!(
                    en: "     Output {artifact}";
                    tr: "     Çıktı {artifact}"
                )
            );
        }
        if sw.check_regmap {
            eprintln!(
                "{}",
                if regmap_checked == 0 {
                    lstr!(
                        en: "       Note: no @mmio module in the unit; --check-regmap checked nothing";
                        tr: "         Not: birimde @mmio modülü yok; --check-regmap hiçbir şey denetlemedi"
                    )
                } else {
                    lstr!(
                        en: "     Regmap drivers match the RTL address decode ({} @mmio module(s))", regmap_checked;
                        tr: "     Regmap sürücüler RTL adres çözümlemesiyle uyumlu ({} @mmio modülü)", regmap_checked
                    )
                }
            );
        }
        // Bağlama göre sonraki adım (UX Anayasası: kullanıcı belgeye
        // gitmeden bir sonraki komutu görür).
        let name = file.display();
        if compiled.modules.is_empty() {
            print_library_note(file);
        } else if project::is_project_mode() {
            eprintln!(
                "{}",
                lstr!(
                    en: "       Next: volt run      (simulate)\n             \
                         volt verify   (prove contracts)";
                    tr: "   Sıradaki: volt run      (simüle et)\n             \
                         volt verify   (kontratları kanıtla)"
                )
            );
        } else {
            eprintln!(
                "{}",
                lstr!(
                    en: "       Next: volt run {name}      (simulate)\n             \
                         volt verify {name}   (prove contracts)";
                    tr: "   Sıradaki: volt run {name}      (simüle et)\n             \
                         volt verify {name}   (kontratları kanıtla)"
                )
            );
        }
    }
    if format == OutputFormat::Json {
        print_json_envelope("build", &compiled, &artifacts, start);
    }
    ExitCode::SUCCESS
}

/// Modülsüz birim (ADR-0042 ek): hata değil — fn/const/tip kütüphanesi
/// başka bir dosyadan `use` ile kullanılır.
fn print_library_note(file: &Path) {
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.display().to_string());
    let stem = file
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    eprintln!(
        "{}",
        lstr!(
            en: "        Note no module in '{name}' — no SystemVerilog written\n        \
                 Help this is a library file: call its pub fn from a module with \
                 'use {stem}::<name>;'";
            tr: "         Not '{name}' içinde modül yok — SystemVerilog yazılmadı\n      \
                 Öneri bu bir kütüphane dosyası: pub fn'lerini bir modülden \
                 'use {stem}::<ad>;' ile çağırın"
        )
    );
}

/// `--emit=rust,c,regmap,regmap-md` (ADR-0053): birimdeki her `@mmio`
/// modülü için istenen türleri yazar, yazılan yolları döndürür. `@mmio`
/// modülü yoksa dosya üretilmez; insan biçiminde bir not düşülür (hata
/// değil — sürücüsü olmayan bir tasarım geçerlidir).
fn write_sw_outputs(
    target_dir: &Path,
    regmaps: &[volt_ast::mmio::RegMap],
    kinds: &[SwKind],
    opts: &EmitOpts,
    format: OutputFormat,
) -> Result<Vec<String>, ExitCode> {
    if regmaps.is_empty() {
        if format == OutputFormat::Human {
            let flags: Vec<&str> = kinds.iter().map(|k| k.flag()).collect();
            eprintln!(
                "{}",
                lstr!(
                    en: "       Note: no @mmio module in the unit; --emit={} produced nothing", flags.join(",");
                    tr: "         Not: birimde @mmio modülü yok; --emit={} hiçbir şey üretmedi", flags.join(",")
                )
            );
        }
        return Ok(Vec::new());
    }
    let mut written = Vec::new();
    for map in regmaps {
        for &kind in kinds {
            let path = kind.output_path(target_dir, map);
            let dir = path.parent().expect("çıktı yolu bir dizin içinde");
            if let Err(err) = std::fs::create_dir_all(dir) {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: cannot create '{}': {}", dir.display(), err;
                        tr: "hata: '{}' oluşturulamadı: {}", dir.display(), err
                    )
                );
                return Err(ExitCode::from(3));
            }
            if let Err(err) = std::fs::write(&path, kind.render(map, opts)) {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: cannot write '{}': {}", path.display(), err;
                        tr: "hata: '{}' yazılamadı: {}", path.display(), err
                    )
                );
                return Err(ExitCode::from(3));
            }
            written.push(path.display().to_string());
        }
    }
    Ok(written)
}

/// `--emit=sdc,xdc` (ADR-0054): saat portu olan her modül için istenen
/// lehçeleri `build/constraints/<Modül>.<sdc|xdc>` olarak yazar, yazılan
/// yolları döndürür. Saatli modül yoksa `Note:` satırı, çıkış 0.
fn write_constraint_outputs(
    target_dir: &Path,
    constraints: &volt_hir::ConstraintResult,
    dialects: &[Dialect],
    opts: &volt_sdc_emit::EmitOpts,
    format: OutputFormat,
) -> Result<Vec<String>, ExitCode> {
    if constraints.modules.is_empty() {
        if format == OutputFormat::Human {
            let flags: Vec<&str> = dialects.iter().map(|d| d.flag()).collect();
            eprintln!(
                "{}",
                lstr!(
                    en: "       Note: no module with a clock port in the unit; --emit={} produced nothing", flags.join(",");
                    tr: "         Not: birimde saat portlu modül yok; --emit={} hiçbir şey üretmedi", flags.join(",")
                )
            );
        }
        return Ok(Vec::new());
    }
    let dir = target_dir.join("constraints");
    if let Err(err) = std::fs::create_dir_all(&dir) {
        eprintln!(
            "{}",
            lstr!(
                en: "error: cannot create '{}': {}", dir.display(), err;
                tr: "hata: '{}' oluşturulamadı: {}", dir.display(), err
            )
        );
        return Err(ExitCode::from(3));
    }
    let mut written = Vec::new();
    for mc in &constraints.modules {
        for &dialect in dialects {
            let path = dialect.output_path(target_dir, &mc.module);
            if let Err(err) = std::fs::write(&path, dialect.render(mc, opts)) {
                eprintln!(
                    "{}",
                    lstr!(
                        en: "error: cannot write '{}': {}", path.display(), err;
                        tr: "hata: '{}' yazılamadı: {}", path.display(), err
                    )
                );
                return Err(ExitCode::from(3));
            }
            written.push(path.display().to_string());
        }
    }
    Ok(written)
}

fn check(file: &Path, format: OutputFormat) -> ExitCode {
    let start = Instant::now();
    if format == OutputFormat::Human {
        eprintln!(
            "{}",
            lstr!(
                en: "    Checking {}", file.display();
                tr: "    Kontrol {}", file.display()
            )
        );
    }

    let compiled = match compile(file, false, SvaMode::None) {
        Ok(c) => c,
        Err(code) => return code,
    };
    render_diagnostics(&compiled, format);

    if format == OutputFormat::Human {
        print_check_footer(start, compiled.errors(), compiled.warnings(), Some(file));
    }
    if format == OutputFormat::Json {
        print_json_envelope("check", &compiled, &[], start);
    }
    if compiled.errors() > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// `check` kapanışı: süre, toplamlar ve hatasızsa sonraki adım. `file`
/// yoksa proje kipi — sonraki komut dosya adı almaz (ADR-0095).
fn print_check_footer(start: Instant, errors: usize, warnings: usize, file: Option<&Path>) {
    eprintln!(
        "{}",
        lstr!(
            en: "    Finished {:.2}s", start.elapsed().as_secs_f64();
            tr: "    Tamamlandı {:.2}s", start.elapsed().as_secs_f64()
        )
    );
    eprintln!(
        "{}",
        lstr!(
            en: "      Result {} error(s), {} warning(s)", errors, warnings;
            tr: "       Sonuç {} hata, {} uyarı", errors, warnings
        )
    );
    if errors > 0 {
        return;
    }
    match file {
        // Test dosyasından SV üretilmez; sıradaki adım testleri koşmak.
        Some(file) if sim::is_test_file(file) => {
            let name = file.display();
            eprintln!(
                "{}",
                lstr!(
                    en: "       Next: volt test {name}   (run the tests)";
                    tr: "   Sıradaki: volt test {name}   (testleri koştur)"
                )
            );
        }
        Some(file) => {
            let name = file.display();
            eprintln!(
                "{}",
                lstr!(
                    en: "       Next: volt build {name}   (emit SystemVerilog)";
                    tr: "   Sıradaki: volt build {name}   (SystemVerilog üret)"
                )
            );
        }
        None => eprintln!(
            "{}",
            lstr!(
                en: "       Next: volt build   (emit SystemVerilog)";
                tr: "   Sıradaki: volt build   (SystemVerilog üret)"
            )
        ),
    }
}

/// Argümansız `volt check` (ADR-0095): projenin her kaynağı, bir kez.
/// Başka bir kaynağın `use` ile yüklediği dosya o birimde denetlenir
/// (kütüphane dosyası tamamen denetlenir, ADR-0070) — tanılar iki kez
/// basılmaz. `--format json`: dosya başına bir zarf, satır satır.
fn check_project(format: OutputFormat) -> ExitCode {
    let project = match project::load("check") {
        Ok(p) => p,
        Err(code) => return code,
    };
    let start = Instant::now();
    let (mut errors, mut warnings) = (0, 0);
    // Test dosyaları kaynak değildir ama denetlenir (kardeş tasarımla);
    // `volt test` ile aynı keşif ve aynı yollar.
    let files = project::check_roots(&project)
        .into_iter()
        .chain(sim::discover_test_files());
    for file in files {
        if format == OutputFormat::Human {
            eprintln!(
                "{}",
                lstr!(
                    en: "    Checking {}", file.display();
                    tr: "    Kontrol {}", file.display()
                )
            );
        }
        let compiled = match compile(&file, false, SvaMode::None) {
            Ok(c) => c,
            Err(code) => return code,
        };
        render_diagnostics(&compiled, format);
        errors += compiled.errors();
        warnings += compiled.warnings();
        if format == OutputFormat::Json {
            print_json_envelope("check", &compiled, &[], start);
        }
    }
    if format == OutputFormat::Human {
        print_check_footer(start, errors, warnings, None);
    }
    if errors > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

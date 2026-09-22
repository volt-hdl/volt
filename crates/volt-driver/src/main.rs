//! Volt CLI — cli-contract.md sözleşmesine göre.
//!
//! `build` ve `check` tam anlamsal boru hattı koşar:
//! parse → resolve → typeck (+const eval) → domain → emit.
//! Bir aşamada hata varsa SONRAKİNE GEÇİLMEZ (kaskad tanı önlemi);
//! CDC ihlali (E3001) derlemeyi durdurur — Volt'un vaadi.
//!
//! Çıkış kodları §2: 0 başarı, 1 derleme hatası, 2 kullanım hatası
//! (clap), 3 G/Ç hatası. Formatlar §5: human | json | short.

mod regmap_check;
mod sim;
mod sim_lower;
mod unit;
mod verify;
mod verify_jobs;
mod verify_report;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
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
    ConstArrayStyle, SbyEngine, SbyMode, SbyOptions, SourceText, SvModule, SvaFile, SvaMode,
    SvaProp,
};
use volt_sw_emit::{EmitOpts, SwKind};
use volt_syntax::ParseResult;

#[derive(Parser)]
#[command(name = "volt", version = volt_sv_emit::VOLT_VERSION)]
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
    #[command(subcommand)]
    command: Option<Command>,
}

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
    volt build --format json --target-dir out design.volt")]
    Build {
        /// Input .volt file
        file: PathBuf,
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
    volt check --format short design.volt")]
    Check {
        /// Input .volt file
        file: PathBuf,
        /// Output format: human | json | short
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Formally verify contracts with SymbiYosys (F4b; exit 6 on counterexample)
    #[command(after_help = "EXAMPLES:
    volt verify design.volt
    volt verify --mode prove design.volt
    volt verify --depth 40 --engine boolector design.volt
    volt verify -j 8 examples/soc/top.volt
    volt verify -j 1 --fail-fast design.volt")]
    Verify {
        /// Input .volt file
        file: PathBuf,
        /// Search depth in cycles (BMC bound / induction length)
        #[arg(long, default_value_t = 20)]
        depth: u32,
        /// Parallel sby jobs: a number or 'auto' (= CPU count); 1 runs modules sequentially
        #[arg(short = 'j', long, default_value = "auto", value_parser = verify_jobs::parse_jobs)]
        jobs: verify_jobs::Jobs,
        /// Stop at the first counterexample (default: every module task completes)
        #[arg(long)]
        fail_fast: bool,
        /// SMT engine: z3 | boolector | yices
        #[arg(long, value_enum, default_value_t = EngineArg::Z3)]
        engine: EngineArg,
        /// Verification mode: bmc | prove | cover
        #[arg(long, value_enum, default_value_t = VerifyModeArg::Bmc)]
        mode: VerifyModeArg,
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
    volt run --vcd waves.vcd design.volt")]
    Run {
        /// Input .volt file
        file: PathBuf,
        /// Number of clock cycles to simulate
        #[arg(long, default_value_t = 100)]
        cycles: u64,
        /// Write a VCD waveform to this file
        #[arg(long)]
        vcd: Option<PathBuf>,
        /// Top module (default: the only module in the file)
        #[arg(long)]
        top: Option<String>,
        /// Output directory (default: build/)
        #[arg(long, default_value = "build")]
        target_dir: PathBuf,
    },
    /// Run simulation tests with Verilator (ADR-0033; cli-contract.md §8)
    #[command(after_help = "EXAMPLES:
    volt test
    volt test my_design_test.volt
    volt test uart --nocapture")]
    Test {
        /// A .volt test file, or a substring filter over test names
        filter: Option<String>,
        /// Also stream the raw testbench output
        #[arg(long)]
        nocapture: bool,
        /// Output directory (default: build/)
        #[arg(long, default_value = "build")]
        target_dir: PathBuf,
    },
    /// Start the Volt language server on stdio (F5a; editors connect here)
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
        /// Color output: auto | always | never (default: VOLT_COLOR or auto)
        #[arg(long, value_enum)]
        color: Option<ColorArg>,
    },
}

/// cli-contract.md §10: --color varsayılanı VOLT_COLOR'dan gelir.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ColorArg {
    Auto,
    Always,
    Never,
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
    Z3,
    Boolector,
    Yices,
}

impl From<EngineArg> for SbyEngine {
    fn from(a: EngineArg) -> Self {
        match a {
            EngineArg::Z3 => SbyEngine::Z3,
            EngineArg::Boolector => SbyEngine::Boolector,
            EngineArg::Yices => SbyEngine::Yices,
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
    let cli = Cli::parse();
    volt_diagnostics::set_lang(resolve_lang(cli.lang));
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
            build(
                &file,
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
            )
        }
        Command::Check { file, format } => check(&file, format),
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
            target_dir,
            format,
        } => verify::verify(
            &file,
            &target_dir,
            format,
            SbyOptions {
                mode: mode.into(),
                depth,
                engine: engine.into(),
                // Görev başına verify.rs'te ayarlanır (multiclock_modules).
                multiclock: false,
            },
            verify::VerifyArgs { jobs, fail_fast },
        ),
        Command::Run {
            file,
            cycles,
            vcd,
            top,
            target_dir,
        } => sim::run(&file, cycles, vcd.as_deref(), top.as_deref(), &target_dir),
        Command::Test {
            filter,
            nocapture,
            target_dir,
        } => sim::test(filter.as_deref(), nocapture, &target_dir),
        Command::Lsp => {
            volt_lsp::run_stdio();
            ExitCode::SUCCESS
        }
        Command::Explain {
            code,
            list,
            topics,
            color,
        } => explain_cmd(code.as_deref(), list, topics, color),
    }
}

/// `volt` (argümansız): sürüm + sık görevler, stdout'a, çıkış 0.
fn print_no_command_help() {
    let version = volt_sv_emit::VOLT_VERSION;
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
fn explain_cmd(code: Option<&str>, list: bool, topics: bool, color: Option<ColorArg>) -> ExitCode {
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
        if let Some(page) =
            explain::topics::render_topic(input, lang, terminal_width(), use_color(color))
        {
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
        explain::render_explanation(parsed, lang, terminal_width(), use_color(color))
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

/// Renk kararı: --color > VOLT_COLOR > auto. Auto modda NO_COLOR ve
/// CI=true rengi kapatır (§10), çıktı terminal değilse de kapalıdır.
fn use_color(flag: Option<ColorArg>) -> bool {
    use std::io::IsTerminal;
    let mode = flag
        .or_else(|| {
            std::env::var("VOLT_COLOR").ok().and_then(|v| {
                match v.trim().to_ascii_lowercase().as_str() {
                    "auto" => Some(ColorArg::Auto),
                    "always" => Some(ColorArg::Always),
                    "never" => Some(ColorArg::Never),
                    _ => None,
                }
            })
        })
        .unwrap_or(ColorArg::Auto);
    match mode {
        ColorArg::Always => true,
        ColorArg::Never => false,
        ColorArg::Auto => {
            std::env::var_os("NO_COLOR").is_none()
                && std::env::var_os("CI").is_none()
                && std::io::stdout().is_terminal()
        }
    }
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
    /// `--emit=sva` ayrı modunda kontratlı modüllerin .sva içerikleri.
    sva_files: Vec<SvaFile>,
    /// Üretilen property kimlikleri (F4b `verify` — sby FAIL eşlemesi).
    sva_props: Vec<SvaProp>,
    /// İki+ saat portlu modüller — `.sby`'ye `multiclock on` (ADR-0027).
    multiclock_modules: Vec<String>,
    /// `@mmio` modüllerinin register haritaları (ADR-0053) — `--emit=rust,
    /// c,regmap,regmap-md` bunlardan üretilir; hata varsa boş.
    regmaps: Vec<volt_ast::mmio::RegMap>,
    /// Zamanlama kısıtı modeli (ADR-0054) — `--emit=sdc,xdc` bundan
    /// üretilir; anlamsal aşama hatalıysa boş.
    constraints: volt_hir::ConstraintResult,
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
/// `check` emit koşmaz (§6 "çıktı üretmeden doğrulama") — sv-emit'in
/// F0 sınırları (örn. sync() çağrısı E0003) analizi engellememeli.
fn compile(file: &Path, want_sv: bool, sva_mode: SvaMode) -> Result<Compiled, ExitCode> {
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
    let map = unit.map;
    let parsed = unit.parsed;
    let file_count = unit.files.len();
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
        sva_files: Vec::new(),
        sva_props: Vec::new(),
        multiclock_modules: Vec::new(),
        regmaps: Vec::new(),
        constraints: Default::default(),
    };
    if count_errors(&diagnostics) > 0 {
        return Ok(fail(map, diagnostics, parsed.ast));
    }

    // ── Aşama 1b: uygulanmayan nitelikler (ADR-0048, W0021) — Volt.toml
    // `[lint] unenforced_attributes = "allow"` ile susturulabilir.
    diagnostics.extend(volt_hir::check_attributes(&parsed.ast, lint_unenforced));

    // ── Aşama 2a: import çözümlemesi — bulunamayan dosya (E1011),
    // döngü (E1006), özel öğe (E1004), belirsizlik (E1010) ──
    diagnostics.extend(unit.diagnostics);
    let imports = volt_hir::check_imports(&parsed.ast, &unit.info);
    diagnostics.extend(imports.diagnostics);
    if count_errors(&diagnostics) > 0 {
        return Ok(fail(map, diagnostics, parsed.ast));
    }

    // Test veri dosyaları (ADR-0058) ana dosyaya göre çözülür.
    let test_files = sim_lower::FsTestFiles::for_test_file(file);
    let Some(constraints) =
        run_semantic_stages(&parsed, &imports.scopes, &test_files, &mut diagnostics)
    else {
        return Ok(fail(map, diagnostics, parsed.ast));
    };
    if count_errors(&diagnostics) > 0 || !want_sv {
        return Ok(fail(map, diagnostics, parsed.ast));
    }

    // ── Aşama 5: emit (E2005 literal boyutlandırma; F4a SVA) ──
    let source_name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.display().to_string());
    let names: Vec<(FileId, String)> = unit
        .files
        .iter()
        .map(|(fid, p)| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string());
            (*fid, name)
        })
        .collect();
    // Ana dosya birimde sonda; SourceText listesinde İLK olmalı (yedek).
    let mut sources: Vec<SourceText<'_>> = names
        .iter()
        .map(|(fid, name)| SourceText {
            file: *fid,
            name,
            text: map.source(*fid),
        })
        .collect();
    sources.rotate_right(1);
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
    let (sv, modules, sva_files, sva_props, multiclock_modules) = if count_errors(&diagnostics) == 0
    {
        (
            Some(emitted.sv),
            emitted.modules,
            emitted.sva_files,
            emitted.sva_props,
            emitted.multiclock_modules,
        )
    } else {
        (None, Vec::new(), Vec::new(), Vec::new(), Vec::new())
    };

    Ok(Compiled {
        map,
        diagnostics,
        ast: parsed.ast,
        sv,
        modules,
        file_count,
        sva_files,
        sva_props,
        multiclock_modules,
        regmaps: parsed.regmaps,
        constraints,
    })
}

/// Aşama 2-4: resolve → const+typeck → domain. Tanılar `out`'a
/// eklenir; bir aşama hata üretirse sonrakiler koşmaz (`None`).
/// Başarıda zamanlama kısıtı modeli (ADR-0054) döner.
fn run_semantic_stages(
    parsed: &ParseResult,
    scopes: &std::collections::HashMap<FileId, FileScope>,
    test_files: &dyn volt_hir::TestFileLoader,
    out: &mut Vec<Diagnostic>,
) -> Option<volt_hir::ConstraintResult> {
    // ── Aşama 2: isim çözümleme (birim modu, ADR-0042) ──
    let resolve = volt_hir::resolve_unit(&parsed.ast, scopes);
    let resolve_failed = count_errors(&resolve.diagnostics) > 0;
    // ADR-0065: ham reset portu senkronizörce örtük okunur (W1001 değil).
    out.extend(volt_hir::without_raw_reset_unused(
        &parsed.ast,
        &resolve.diagnostics,
    ));
    if resolve_failed {
        return None;
    }

    // ── Aşama 3: const eval + tip kontrolü ──
    let mut evaluator = volt_hir::ConstEvaluator::new(&parsed.ast, &resolve);
    evaluator.eval_all_consts();
    evaluator.check_type_positions();
    let typeck = volt_hir::typecheck(&parsed.ast, &resolve, &mut evaluator);
    let stage_failed =
        count_errors(&evaluator.diagnostics) > 0 || count_errors(&typeck.diagnostics) > 0;
    out.extend(evaluator.diagnostics.iter().cloned());
    out.extend(typeck.diagnostics.iter().cloned());
    if stage_failed {
        return None;
    }

    // ── Aşama 4: domain çıkarımı ve CDC (Volt'un vaadi) ──
    let domain = volt_hir::infer_domains(&parsed.ast, &resolve, &typeck);
    // ── F2f güven seviyeleri (ADR-0052): E3009 / W3008 ──
    let trust = volt_hir::check_trust(&parsed.ast, &resolve, &typeck, &domain);
    // ── Reset alanı denetimi (ADR-0065): E3003 / W3009 / W3010 ──
    let rdc = volt_hir::check_rdc(&parsed.ast, &resolve, &domain);
    out.extend(domain.diagnostics);
    out.extend(trust);
    out.extend(rdc);

    // ── L1 zamanlama (ADR-0037): yalnız @strict_timing modülleri ──
    out.extend(volt_hir::check_timing(&parsed.ast, &resolve));

    // ── Handshake protokolü (ADR-0050): valid, ready'ye bağlı olamaz ──
    out.extend(volt_hir::check_handshakes(&parsed.ast, &resolve));

    // ── Test blokları (ADR-0033) ── Dosyada hiç modül yoksa testler
    // kardeş dosyanın modüllerini kullanıyordur; modül-varlık denetimi
    // atlanır (sim.rs kardeş dosyayla tam denetimi yapar).
    let has_modules = parsed.ast.items.iter().any(|i| {
        matches!(
            parsed.ast.items_arena[*i].kind,
            volt_ast::ItemKind::Module(_)
        )
    });
    out.extend(volt_hir::check_tests_with_files(
        &[&parsed.ast],
        &parsed.ast,
        !has_modules,
        Some(test_files),
    ));

    // ── Zamanlama kısıtları (ADR-0054): E0017 her zaman; model
    // `--emit=sdc,xdc` için saklanır, W0022 orada üretilir ──
    let constraints = volt_hir::collect_constraints(&parsed.ast);
    out.extend(constraints.diagnostics.iter().cloned());
    Some(constraints)
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
    // `--single-file` eski düzeni (build/rtl/<kaynak>.sv) korur.
    let outputs: Vec<(PathBuf, &str)> = if single_file || compiled.modules.is_empty() {
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
            version: volt_sv_emit::VOLT_VERSION.to_string(),
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
    if format == OutputFormat::Json {
        print_json_envelope("build", &compiled, &artifacts, start);
    }
    ExitCode::SUCCESS
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
                en: "      Result {} error(s), {} warning(s)",
                    compiled.errors(), compiled.warnings();
                tr: "       Sonuç {} hata, {} uyarı",
                    compiled.errors(), compiled.warnings()
            )
        );
        if compiled.errors() == 0 {
            let name = file.display();
            eprintln!(
                "{}",
                lstr!(
                    en: "       Next: volt build {name}   (emit SystemVerilog)";
                    tr: "   Sıradaki: volt build {name}   (SystemVerilog üret)"
                )
            );
        }
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

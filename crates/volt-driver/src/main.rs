//! Volt CLI — cli-contract.md sözleşmesine göre.
//!
//! `build` ve `check` tam anlamsal boru hattı koşar:
//! parse → resolve → typeck (+const eval) → domain → emit.
//! Bir aşamada hata varsa SONRAKİNE GEÇİLMEZ (kaskad tanı önlemi);
//! CDC ihlali (E3001) derlemeyi durdurur — Volt'un vaadi.
//!
//! Çıkış kodları §2: 0 başarı, 1 derleme hatası, 2 kullanım hatası
//! (clap), 3 G/Ç hatası. Formatlar §5: human | json | short.

mod sim;
mod verify;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use volt_diagnostics::{
    explain, lstr, render_human, render_short, to_json_value, Diagnostic, ErrorCode, Lang, Severity,
};
use volt_span::SourceMap;
use volt_sv_emit::{SbyEngine, SbyMode, SbyOptions, SvaFile, SvaMode, SvaProp};
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
        /// Additional outputs: sva (SystemVerilog assertions from contracts)
        #[arg(long, value_enum, value_delimiter = ',')]
        emit: Vec<EmitArg>,
        /// SVA placement: separate .sva file with bind | inline in the .sv
        #[arg(long, value_enum, default_value_t = SvaArg::Separate)]
        sva: SvaArg,
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
    volt verify --depth 40 --engine boolector design.volt")]
    Verify {
        /// Input .volt file
        file: PathBuf,
        /// Search depth in cycles (BMC bound / induction length)
        #[arg(long, default_value_t = 20)]
        depth: u32,
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

/// `--emit` ek çıktıları (F4a).
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum EmitArg {
    Sva,
}

/// `--sva` yerleşimi (F4a ADIM 3); varsayılan ayrı dosya + bind.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SvaArg {
    Separate,
    Inline,
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
        } => {
            let mode = if emit.contains(&EmitArg::Sva) {
                match sva {
                    SvaArg::Separate => SvaMode::Separate,
                    SvaArg::Inline => SvaMode::Inline,
                }
            } else {
                SvaMode::None
            };
            build(&file, &target_dir, format, mode)
        }
        Command::Check { file, format } => check(&file, format),
        Command::Verify {
            file,
            depth,
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
                // Modül başına verify.rs'te ayarlanır (multiclock_modules).
                multiclock: false,
            },
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
    /// `--emit=sva` ayrı modunda kontratlı modüllerin .sva içerikleri.
    sva_files: Vec<SvaFile>,
    /// Üretilen property kimlikleri (F4b `verify` — sby FAIL eşlemesi).
    sva_props: Vec<SvaProp>,
    /// İki+ saat portlu modüller — `.sby`'ye `multiclock on` (ADR-0027).
    multiclock_modules: Vec<String>,
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
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
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

    let mut map = SourceMap::new();
    let file_id = map.add_file(file.display().to_string(), source.clone());

    // ── Aşama 1: parse ──
    let parsed = volt_syntax::parser::parse(file_id, &source);
    let mut diagnostics = parsed.diagnostics.clone();
    if count_errors(&diagnostics) > 0 {
        return Ok(Compiled {
            map,
            diagnostics,
            ast: parsed.ast,
            sv: None,
            sva_files: Vec::new(),
            sva_props: Vec::new(),
            multiclock_modules: Vec::new(),
        });
    }

    let semantic = run_semantic_stages(&parsed, &mut diagnostics);
    if count_errors(&diagnostics) > 0 || !semantic || !want_sv {
        return Ok(Compiled {
            map,
            diagnostics,
            ast: parsed.ast,
            sv: None,
            sva_files: Vec::new(),
            sva_props: Vec::new(),
            multiclock_modules: Vec::new(),
        });
    }

    // ── Aşama 5: emit (E2005 literal boyutlandırma; F4a SVA) ──
    let source_name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.display().to_string());
    let emitted = volt_sv_emit::emit_full(&parsed.ast, &source_name, &source, sva_mode);
    diagnostics.extend(emitted.diagnostics);
    let (sv, sva_files, sva_props, multiclock_modules) = if count_errors(&diagnostics) == 0 {
        (
            Some(emitted.sv),
            emitted.sva_files,
            emitted.sva_props,
            emitted.multiclock_modules,
        )
    } else {
        (None, Vec::new(), Vec::new(), Vec::new())
    };

    Ok(Compiled {
        map,
        diagnostics,
        ast: parsed.ast,
        sv,
        sva_files,
        sva_props,
        multiclock_modules,
    })
}

/// Aşama 2-4: resolve → const+typeck → domain. Tanılar `out`'a
/// eklenir; bir aşama hata üretirse sonrakiler koşmaz.
fn run_semantic_stages(parsed: &ParseResult, out: &mut Vec<Diagnostic>) -> bool {
    // ── Aşama 2: isim çözümleme ──
    let resolve = volt_hir::resolve_file(&parsed.ast);
    let resolve_failed = count_errors(&resolve.diagnostics) > 0;
    out.extend(resolve.diagnostics.iter().cloned());
    if resolve_failed {
        return false;
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
        return false;
    }

    // ── Aşama 4: domain çıkarımı ve CDC (Volt'un vaadi) ──
    let domain = volt_hir::infer_domains(&parsed.ast, &resolve, &typeck);
    out.extend(domain.diagnostics);

    // ── Test blokları (ADR-0033) ── Dosyada hiç modül yoksa testler
    // kardeş dosyanın modüllerini kullanıyordur; modül-varlık denetimi
    // atlanır (sim.rs kardeş dosyayla tam denetimi yapar).
    let has_modules = parsed.ast.items.iter().any(|i| {
        matches!(
            parsed.ast.items_arena[*i].kind,
            volt_ast::ItemKind::Module(_)
        )
    });
    out.extend(volt_hir::check_tests(
        &[&parsed.ast],
        &parsed.ast,
        !has_modules,
    ));
    true
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
    // CI `diagnostics[0]`'da engelleyici hatayı bekler: hatalar önce,
    // uyarılar sonra (kendi içlerinde kaynak sırası korunur).
    let mut ordered: Vec<&Diagnostic> = compiled.diagnostics.iter().collect();
    ordered.sort_by_key(|d| d.severity != Severity::Error);
    let envelope = serde_json::json!({
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
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&envelope).expect("JSON zarfı")
    );
}

fn build(file: &Path, target_dir: &Path, format: OutputFormat, sva_mode: SvaMode) -> ExitCode {
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

    let compiled = match compile(file, true, sva_mode) {
        Ok(c) => c,
        Err(code) => return code,
    };
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
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let out_path = rtl_dir.join(format!("{stem}.sv"));
    if let Err(err) = std::fs::write(&out_path, sv) {
        eprintln!(
            "{}",
            lstr!(
                en: "error: cannot write '{}': {}", out_path.display(), err;
                tr: "hata: '{}' yazılamadı: {}", out_path.display(), err
            )
        );
        return ExitCode::from(3);
    }

    // F4a — ayrı SVA dosyaları: build/formal/<modul>.sva.
    let mut artifacts = vec![out_path.display().to_string()];
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
                en: "     Output {} ({} lines)", out_path.display(), sv.lines().count();
                tr: "     Çıktı {} ({} satır)", out_path.display(), sv.lines().count()
            )
        );
        for artifact in artifacts.iter().skip(1) {
            eprintln!(
                "{}",
                lstr!(
                    en: "     Output {artifact}";
                    tr: "     Çıktı {artifact}"
                )
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

//! Volt CLI — cli-contract.md sözleşmesine göre.
//!
//! `build` ve `check` tam anlamsal boru hattı koşar:
//! parse → resolve → typeck (+const eval) → domain → emit.
//! Bir aşamada hata varsa SONRAKİNE GEÇİLMEZ (kaskad tanı önlemi);
//! CDC ihlali (E3001) derlemeyi durdurur — Volt'un vaadi.
//!
//! Çıkış kodları §2: 0 başarı, 1 derleme hatası, 2 kullanım hatası
//! (clap), 3 G/Ç hatası. Formatlar §5: human | json | short.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use volt_diagnostics::{render_human, render_short, to_json_value, Diagnostic, Severity};
use volt_span::SourceMap;
use volt_syntax::ParseResult;

#[derive(Parser)]
#[command(name = "volt", version = volt_sv_emit::VOLT_VERSION)]
#[command(about = "Volt HDL — saat alanı güvenli donanım tanımlama dili")]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
    /// Derle ve SystemVerilog üret
    Build {
        /// Girdi .volt dosyası
        file: PathBuf,
        /// Çıktı dizini (varsayılan: build/)
        #[arg(long, default_value = "build")]
        target_dir: PathBuf,
        /// Çıktı formatı: human | json | short
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Hızlı kontrol (çıktı üretmez)
    Check {
        /// Girdi .volt dosyası
        file: PathBuf,
        /// Çıktı formatı: human | json | short
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Build {
            file,
            target_dir,
            format,
        } => build(&file, &target_dir, format),
        Command::Check { file, format } => check(&file, format),
    }
}

struct Compiled {
    map: SourceMap,
    diagnostics: Vec<Diagnostic>,
    /// Yalnız tüm aşamalar hatasızsa üretilir.
    sv: Option<String>,
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
fn compile(file: &Path, want_sv: bool) -> Result<Compiled, ExitCode> {
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("hata: '{}' okunamadı: {err}", file.display());
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
            sv: None,
        });
    }

    let semantic = run_semantic_stages(&parsed, &mut diagnostics);
    if count_errors(&diagnostics) > 0 || !semantic || !want_sv {
        return Ok(Compiled {
            map,
            diagnostics,
            sv: None,
        });
    }

    // ── Aşama 5: emit (E2005 literal boyutlandırma) ──
    let source_name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.display().to_string());
    let emitted = volt_sv_emit::emit(&parsed.ast, &source_name);
    diagnostics.extend(emitted.diagnostics);
    let sv = if count_errors(&diagnostics) == 0 {
        Some(emitted.sv)
    } else {
        None
    };

    Ok(Compiled {
        map,
        diagnostics,
        sv,
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

fn build(file: &Path, target_dir: &Path, format: OutputFormat) -> ExitCode {
    let start = Instant::now();
    if format == OutputFormat::Human {
        eprintln!("   Derleniyor {}", file.display());
    }

    let compiled = match compile(file, true) {
        Ok(c) => c,
        Err(code) => return code,
    };
    render_diagnostics(&compiled, format);

    // Hata varsa SV ÜRETİLMEZ — hatalı tasarım sentezlenemez.
    let Some(sv) = &compiled.sv else {
        if format == OutputFormat::Human {
            eprintln!(
                "     Hata: {} hata, {} uyarı nedeniyle derleme başarısız",
                compiled.errors(),
                compiled.warnings()
            );
        }
        if format == OutputFormat::Json {
            print_json_envelope("build", &compiled, &[], start);
        }
        return ExitCode::from(1);
    };

    let rtl_dir = target_dir.join("rtl");
    if let Err(err) = std::fs::create_dir_all(&rtl_dir) {
        eprintln!("hata: '{}' oluşturulamadı: {err}", rtl_dir.display());
        return ExitCode::from(3);
    }
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let out_path = rtl_dir.join(format!("{stem}.sv"));
    if let Err(err) = std::fs::write(&out_path, sv) {
        eprintln!("hata: '{}' yazılamadı: {err}", out_path.display());
        return ExitCode::from(3);
    }

    if format == OutputFormat::Human {
        let line_count = sv.lines().count();
        eprintln!("    Tamamlandı {:.2}s", start.elapsed().as_secs_f64());
        eprintln!("     Çıktı {} ({} satır)", out_path.display(), line_count);
    }
    if format == OutputFormat::Json {
        print_json_envelope("build", &compiled, &[out_path.display().to_string()], start);
    }
    ExitCode::SUCCESS
}

fn check(file: &Path, format: OutputFormat) -> ExitCode {
    let start = Instant::now();
    if format == OutputFormat::Human {
        eprintln!("    Kontrol {}", file.display());
    }

    let compiled = match compile(file, false) {
        Ok(c) => c,
        Err(code) => return code,
    };
    render_diagnostics(&compiled, format);

    if format == OutputFormat::Human {
        eprintln!("    Tamamlandı {:.2}s", start.elapsed().as_secs_f64());
        eprintln!(
            "       Sonuç {} hata, {} uyarı",
            compiled.errors(),
            compiled.warnings()
        );
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

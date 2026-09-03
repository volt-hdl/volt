//! Volt CLI — cli-contract.md sözleşmesine göre.
//!
//! F0 kapsamı: `build` ve `check`. Çıkış kodları §2:
//! 0 başarı, 1 derleme hatası, 2 kullanım hatası (clap), 3 G/Ç hatası.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};
use volt_diagnostics::{render_human, Severity};
use volt_span::SourceMap;

#[derive(Parser)]
#[command(name = "volt", version = volt_sv_emit::VOLT_VERSION)]
#[command(about = "Volt HDL — saat alanı güvenli donanım tanımlama dili")]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
    },
    /// Hızlı kontrol (çıktı üretmez)
    Check {
        /// Girdi .volt dosyası
        file: PathBuf,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Build { file, target_dir } => build(&file, &target_dir),
        Command::Check { file } => check(&file),
    }
}

struct Compiled {
    sv: String,
    errors: usize,
    warnings: usize,
}

/// Ortak boru hattı: oku → parse → emit → tanıları stderr'e yaz.
fn compile(file: &Path) -> Result<Compiled, ExitCode> {
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("hata: '{}' okunamadı: {err}", file.display());
            return Err(ExitCode::from(3));
        }
    };

    let mut map = SourceMap::new();
    let file_id = map.add_file(file.display().to_string(), source.clone());

    let parsed = volt_syntax::parser::parse(file_id, &source);
    let source_name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.display().to_string());
    let emitted = volt_sv_emit::emit(&parsed.ast, &source_name);

    let mut diagnostics = parsed.diagnostics;
    diagnostics.extend(emitted.diagnostics);

    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();
    for diag in &diagnostics {
        eprintln!("{}", render_human(diag, &map));
    }

    Ok(Compiled {
        sv: emitted.sv,
        errors,
        warnings,
    })
}

fn build(file: &Path, target_dir: &Path) -> ExitCode {
    let start = Instant::now();
    eprintln!("   Derleniyor {}", file.display());

    let compiled = match compile(file) {
        Ok(c) => c,
        Err(code) => return code,
    };

    if compiled.errors > 0 {
        eprintln!(
            "     Hata: {} hata, {} uyarı nedeniyle derleme başarısız",
            compiled.errors, compiled.warnings
        );
        return ExitCode::from(1);
    }

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
    if let Err(err) = std::fs::write(&out_path, &compiled.sv) {
        eprintln!("hata: '{}' yazılamadı: {err}", out_path.display());
        return ExitCode::from(3);
    }

    let line_count = compiled.sv.lines().count();
    eprintln!("    Tamamlandı {:.2}s", start.elapsed().as_secs_f64());
    eprintln!("     Çıktı {} ({} satır)", out_path.display(), line_count);
    ExitCode::SUCCESS
}

fn check(file: &Path) -> ExitCode {
    let start = Instant::now();
    eprintln!("    Kontrol {}", file.display());

    let compiled = match compile(file) {
        Ok(c) => c,
        Err(code) => return code,
    };

    eprintln!("    Tamamlandı {:.2}s", start.elapsed().as_secs_f64());
    eprintln!(
        "       Sonuç {} hata, {} uyarı",
        compiled.errors, compiled.warnings
    );
    if compiled.errors > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

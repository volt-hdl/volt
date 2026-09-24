//! `extern module` SV kaynaklarının araca verilmesi (ADR-0076).
//!
//! `run`/`test`/`verify` örneklenen her extern'ün gövdesini ister
//! (kaynaksızsa E1012) ve `@source` dosyalarını aracın çalışma dizinine
//! kopyalayıp üretilen SV'den önce girdi yapar.

use std::path::Path;
use std::process::ExitCode;

use volt_diagnostics::lstr;
use volt_sv_emit::SvaMode;

use crate::{compile, Compiled};

/// `run`/`test`/`verify` derlemesi (ADR-0076): örneklenen her extern'ün
/// SV kaynağı bilinmeli — yoksa E1012 tanısı eklenir ve SV üretilmemiş
/// sayılır (çağıranların hata yolu değişmez). `build`/`check` bunu
/// istemez: yalnız örneklemeyi üretirler.
pub(crate) fn compile_for_tool(
    file: &Path,
    sva_mode: SvaMode,
    command: &str,
) -> Result<Compiled, ExitCode> {
    let mut compiled = compile(file, true, sva_mode)?;
    if compiled.sv.is_some() {
        let missing = volt_hir::missing_sources(&compiled.ast, command);
        if !missing.is_empty() {
            compiled.diagnostics.extend(missing);
            compiled.sv = None;
            compiled.modules.clear();
        }
    }
    Ok(compiled)
}

/// Extern SV kaynaklarını aracın çalışma dizinine kopyalar (ADR-0076);
/// dönen adlar `dir`'e görelidir ve üretilen SV'den ÖNCE okunmalıdır.
/// Ad `extern_<dosya adı>`; aynı adlı iki farklı dosyada sıra eklenir.
pub(crate) fn stage_extern_sources(
    sources: &[volt_hir::ExternSourceFile],
    dir: &Path,
) -> Result<Vec<String>, ExitCode> {
    let mut names: Vec<String> = Vec::new();
    for (k, src) in sources.iter().enumerate() {
        let base = src
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{}.sv", src.module));
        let mut name = format!("extern_{base}");
        if names.contains(&name) {
            name = format!("extern_{k}_{base}");
        }
        let dest = dir.join(&name);
        if let Err(err) = std::fs::copy(&src.path, &dest) {
            eprintln!(
                "{}",
                lstr!(
                    en: "error: cannot copy '{}' to '{}': {}", src.path.display(), dest.display(), err;
                    tr: "hata: '{}', '{}' konumuna kopyalanamadı: {}", src.path.display(), dest.display(), err
                )
            );
            return Err(ExitCode::from(3));
        }
        names.push(name);
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn src(module: &str, path: PathBuf) -> volt_hir::ExternSourceFile {
        volt_hir::ExternSourceFile {
            module: module.to_string(),
            path,
        }
    }

    /// Adlar `extern_<dosya adı>`; farklı dizinlerde aynı adlı iki dosya
    /// birbirini ezmez (ikincisine sıra eklenir).
    #[test]
    fn staged_names_are_unique_even_for_equal_basenames() {
        let dir = std::env::temp_dir().join(format!("volt-stage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["a", "b", "out"] {
            std::fs::create_dir_all(dir.join(sub)).expect("dizin");
        }
        std::fs::write(dir.join("a").join("fifo.sv"), "module A; endmodule\n").expect("a");
        std::fs::write(dir.join("b").join("fifo.sv"), "module B; endmodule\n").expect("b");
        let sources = [
            src("A", dir.join("a").join("fifo.sv")),
            src("B", dir.join("b").join("fifo.sv")),
        ];
        let names = stage_extern_sources(&sources, &dir.join("out")).expect("kopyalanmalı");
        assert_eq!(names, ["extern_fifo.sv", "extern_1_fifo.sv"]);
        let second = std::fs::read_to_string(dir.join("out").join("extern_1_fifo.sv")).expect("b");
        assert!(second.contains("module B"), "{second}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_source_file_is_io_error_exit_3() {
        let dir = std::env::temp_dir().join(format!("volt-stage-miss-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dizin");
        let sources = [src("A", dir.join("gone.sv"))];
        assert!(stage_extern_sources(&sources, &dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

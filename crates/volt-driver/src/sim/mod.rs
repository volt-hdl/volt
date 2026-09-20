//! `volt run` ve `volt test` — Verilator simülasyon köprüsü (ADR-0033).
//!
//! Akış: derle → SV + C++ testbench üret → `verilator --cc --exe
//! --build` → yürütülebiliri koştur → çıktıyı yorumla. Verilator
//! `VOLT_VERILATOR` > `PATH` sırasıyla aranır (F4b `verify` VOLT_SBY
//! deseni). Çıkış kodları (cli-contract.md §2): 0 başarı, 1 derleme
//! hatası, 2 kullanım hatası, 3 Verilator yok / araç hatası, 5 test
//! koştu ve en az biri kaldı.
//!
//! Cargo biçimli test raporu BİLEREK İngilizcedir (makine-okur veri
//! çıktısı, cargo ile birebir); çevresel iletiler `lstr!` ile yereldir.
//!
//! Modüller sorumluluğa göre ayrılır: `run_cmd` (`volt run` akışı),
//! `test_cmd` (`volt test` akışı), `test_files` (test dosyası keşfi),
//! `test_build` (derleme, denetim, gruplama), `verilator` (araç keşfi,
//! derleme, koşturma), `tb_output` (VOLT-* çıktı protokolü), `report`
//! (cargo biçimli rapor). C++ testbench metni burada değil
//! `volt_sv_emit::sim` içinde üretilir.

mod report;
mod run_cmd;
mod tb_output;
mod test_build;
mod test_cmd;
mod test_files;
mod verilator;

use std::path::Path;
use std::process::ExitCode;

use volt_ast::{ItemKind, ModuleDecl, SourceFile};
use volt_diagnostics::lstr;

pub(crate) use run_cmd::run;
pub(crate) use test_cmd::test;

// ═══ Ortak yardımcılar ════════════════════════════════════════════

fn io_error(path: &Path, err: &std::io::Error) -> ExitCode {
    eprintln!(
        "{}",
        lstr!(
            en: "error: cannot write '{}': {}", path.display(), err;
            tr: "hata: '{}' yazılamadı: {}", path.display(), err
        )
    );
    ExitCode::from(3)
}

/// Sim dizinini (yoksa) oluşturur.
fn create_sim_dir(sim_dir: &Path) -> Result<(), ExitCode> {
    std::fs::create_dir_all(sim_dir).map_err(|err| io_error(sim_dir, &err))
}

/// Üretilen dosyayı (SV, testbench, `.vlt`) sim dizinine yazar.
fn write_file(path: &Path, contents: &str) -> Result<(), ExitCode> {
    std::fs::write(path, contents).map_err(|err| io_error(path, &err))
}

/// AST'deki modülleri kaynak sırasıyla listeler.
fn modules_of(ast: &SourceFile) -> Vec<&ModuleDecl> {
    ast.items
        .iter()
        .filter_map(|i| match &ast.items_arena[*i].kind {
            ItemKind::Module(m) => Some(m),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_file_reports_missing_parent_as_error() {
        let dir = std::env::temp_dir().join(format!("volt-sim-mod-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        // Üst dizin yokken yazım hatadır; oluşturulunca başarılıdır.
        assert!(write_file(&dir.join("tb.cpp"), "x").is_err());
        assert!(create_sim_dir(&dir).is_ok());
        assert!(write_file(&dir.join("tb.cpp"), "x").is_ok());
        assert_eq!(
            std::fs::read_to_string(dir.join("tb.cpp")).expect("oku"),
            "x"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

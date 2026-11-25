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
//! `test_cmd` (`volt test` akışı), `test_files` (süzgeç, kardeş kuralı),
//! `discover` (proje kökünden test dosyası keşfi, ADR-0089),
//! `test_build` (derleme, denetim, gruplama), `verilator` (araç keşfi,
//! derleme, koşturma), `tb_output` (VOLT-* çıktı protokolü), `contracts`
//! (kontrat izleyici çıktısı, ADR-0064), `report` (cargo biçimli rapor). C++ testbench metni burada değil
//! `volt_sv_emit::sim` içinde üretilir.

mod contracts;
mod discover;
mod report;
mod run_cmd;
mod tb_output;
mod test_build;
mod test_cmd;
mod test_files;
mod test_waves;
mod verilator;

use std::path::Path;
use std::process::ExitCode;

use volt_ast::{ItemKind, ModuleDecl, SourceFile};

use crate::output::{Action, OutputRoot};

pub(crate) use discover::{discover_test_files, is_test_file, project_files};
pub(crate) use run_cmd::{run, RunOptions};
pub(crate) use test_cmd::{test, TestOptions};
pub(crate) use test_waves::WaveMode;

// ═══ Ortak yardımcılar ════════════════════════════════════════════

/// Sim dizinini (yoksa) oluşturur; hata iletisi yazım hatasınınkidir.
fn create_sim_dir(out: &OutputRoot, sim_dir: &Path) -> Result<(), ExitCode> {
    out.create_dir_all(sim_dir)
        .map_err(|err| err.report(Action::Write, sim_dir))
}

/// Üretilen dosyayı (SV, testbench, `.vlt`) sim dizinine yazar.
fn write_file(out: &OutputRoot, path: &Path, contents: &str) -> Result<(), ExitCode> {
    out.write(path, contents)
        .map_err(|err| err.report(Action::Write, path))
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
    use crate::output::Origin;

    #[test]
    fn write_file_reports_missing_parent_as_error() {
        let base = std::env::temp_dir().join(format!("volt-sim-mod-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("temp dizini");
        let out = OutputRoot::new(&base, Origin::TargetDir);
        let dir = base.join("sim");

        // Üst dizin yokken yazım hatadır; oluşturulunca başarılıdır.
        assert!(write_file(&out, &dir.join("tb.cpp"), "x").is_err());
        assert!(create_sim_dir(&out, &dir).is_ok());
        assert!(write_file(&out, &dir.join("tb.cpp"), "x").is_ok());
        assert_eq!(
            std::fs::read_to_string(dir.join("tb.cpp")).expect("oku"),
            "x"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}

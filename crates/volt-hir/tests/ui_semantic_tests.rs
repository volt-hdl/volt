//! tests/ui dosyalarının anlamsal (F1b) doğrulaması.
//!
//! Parser seviyesi ui taraması volt-syntax'ta; burada isim çözümleme
//! ve const eval'in ui/fail beklentileri denetlenir.

use volt_hir::analyze;
use volt_syntax::{parse, FileId};

fn analyze_file(rel: &str) -> volt_hir::AnalysisResult {
    let path = format!("{}/../../tests/ui/{rel}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).expect("ui dosyası okunmalı");
    let parsed = parse(FileId(0), &src);
    assert!(
        parsed.diagnostics.is_empty(),
        "{rel} ayrışmalı: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
}

#[test]
fn ui_fail_19_undefined_name_e1001_with_help() {
    let result = analyze_file("fail/19_undefined_name.volt");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E1001")
        .expect("E1001 bekleniyor");
    // 5 parça kuralı: çözüm önerisi her tanıda zorunlu.
    assert!(
        !diag.help.as_deref().unwrap_or("").is_empty(),
        "E1001 yardım metni taşımalı"
    );
}

#[test]
fn ui_fail_21_cyclic_const_e2020() {
    let result = analyze_file("fail/21_cyclic_const.volt");
    assert!(
        result.error_codes().contains(&"E2020"),
        "E2020 bekleniyor: {:?}",
        result.error_codes()
    );
}

#[test]
fn ui_fail_22_runtime_in_type_e2021() {
    let result = analyze_file("fail/22_runtime_in_type.volt");
    assert!(
        result.error_codes().contains(&"E2021"),
        "E2021 bekleniyor: {:?}",
        result.error_codes()
    );
}

#[test]
fn ui_pass_files_have_no_semantic_errors() {
    // Uyarılar serbest; hatalar regresyondur.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui/pass");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).expect("ui/pass okunmalı") {
        let path = entry.expect("girdi").path();
        if path.extension().and_then(|e| e.to_str()) != Some("volt") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("dosya okunmalı");
        let parsed = parse(FileId(0), &src);
        assert!(parsed.diagnostics.is_empty());
        let result = analyze(&parsed.ast);
        assert!(
            !result.has_errors(),
            "{:?} anlamsal hata üretmemeli: {:?}",
            path.file_name(),
            result.error_codes()
        );
        checked += 1;
    }
    assert_eq!(checked, 21);
}

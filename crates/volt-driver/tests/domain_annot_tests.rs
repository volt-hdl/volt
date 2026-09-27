//! Bildirimlerde saat alanı açıklaması (ADR-0088): `wire`, `let` (modül
//! ve blok) ve `reg` `@Alan` taşıyabilir. Açıklama DENETLENİR (çelişki
//! E3001, tanımsız alan E3002, `reg(clk)` ile birlikte E0001) ve üretilen
//! SV'yi değiştirmez. Önce `wire x : T @Alan` sonraki satırın bilinmeyen
//! niteliği sayılıp W0020 ile düşüyordu — hiçbir şey denetlenmiyordu.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const TAG: &str = "_domain_annot_";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fail_fixtures() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join("tests/ui/fail"))
        .expect("ui/fail okunmalı")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().contains(TAG))
        })
        .collect();
    files.sort();
    files
}

fn check_json(file: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(file)
        .output()
        .expect("volt çalışmalı");
    serde_json::from_slice(&out.stdout).expect("JSON")
}

#[test]
fn every_fail_fixture_reports_exactly_its_code_on_the_marked_line() {
    let files = fail_fixtures();
    assert_eq!(files.len(), 4, "ADR-0088 ui/fail fixture sayısı");
    let mut bad = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("okunmalı");
        let code = text
            .lines()
            .next()
            .and_then(|l| l.trim().strip_prefix("//~ "))
            .expect("ilk satır '//~ KOD'")
            .trim()
            .to_string();
        let line = text
            .lines()
            .position(|l| l.trim_start().starts_with("//~^ ERROR"))
            .expect("işaret") as u64;
        let errs: Vec<(String, u64)> = check_json(file)["diagnostics"]
            .as_array()
            .expect("dizi")
            .iter()
            .filter(|d| d["severity"] == "error")
            .map(|d| {
                let l = d["spans"]
                    .as_array()
                    .and_then(|s| s.iter().find(|s| s["primary"] == true))
                    .and_then(|s| s["start"]["line"].as_u64())
                    .unwrap_or(0);
                (d["code"].as_str().unwrap_or_default().to_string(), l)
            })
            .collect();
        if errs != [(code.clone(), line)] {
            bad.push(format!(
                "{}: yalnız {code}@{line}, bulunan {errs:?}",
                file.display()
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Kaynağı derler, `Annotated.sv`'yi döndürür.
fn build_sv(tag: &str, src: &str) -> String {
    let dir = std::env::temp_dir().join(format!("volt-dann-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp");
    let file = dir.join("annotated.volt");
    std::fs::write(&file, src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--target-dir"])
        .arg(dir.join("out"))
        .arg(&file)
        .output()
        .expect("volt çalışmalı");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && !stderr.contains("warning["),
        "{stderr}"
    );
    let sv = std::fs::read_to_string(dir.join("out/rtl/Annotated.sv")).expect("SV");
    let _ = std::fs::remove_dir_all(&dir);
    sv
}

#[test]
fn annotations_change_no_generated_systemverilog() {
    let annotated =
        std::fs::read_to_string(root().join("tests/ui/pass/130_domain_annotations.volt"))
            .expect("fixture");
    let stripped = annotated
        .replace(" : bool @Slow", " : bool")
        .replace(" : bool @Fast = ", " : bool = ")
        .replace(" : u4 @Slow = ", " : u4 = ");
    assert!(
        !stripped.contains("bool @Slow\n    synced"),
        "açıklamalar kalktı"
    );
    assert_ne!(annotated, stripped);
    assert_eq!(build_sv("a", &annotated), build_sv("s", &stripped));
}

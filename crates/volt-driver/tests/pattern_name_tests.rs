//! Desendeki çıplak ad bir DEĞERDİR (ADR-0085): `tests/ui/fail/*_pattern_name_*`
//! fixture'ları `volt check`'te beklenen kodu beklenen satırda verir;
//! `tests/ui/pass/*_pattern_name_*` SV'sinde const adı sınananın
//! genişliğinde `case` etiketi olur, hiçbir kol "her şeyi yakalayan"
//! bağlamaya dönüşmez.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const TAG: &str = "_pattern_name_";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
        .expect("ui dizini okunmalı")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|e| e == "volt")
                && p.file_name()
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
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "{}: JSON değil ({e}): {}",
            file.display(),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn diagnostics(env: &Value) -> &[Value] {
    env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
}

fn primary_line(d: &Value) -> u64 {
    d["spans"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["primary"] == true))
        .and_then(|s| s["start"]["line"].as_u64())
        .unwrap_or(0)
}

/// Satır 1 `//~ KOD`; `//~^ ERROR` bir üst satırı işaretler.
fn expectation(text: &str) -> (String, u64) {
    let code = text
        .lines()
        .next()
        .and_then(|l| l.trim().strip_prefix("//~ "))
        .map(|c| c.trim().to_string())
        .expect("ilk satır '//~ KOD'");
    let line = text
        .lines()
        .position(|l| l.trim_start().starts_with("//~^ ERROR"))
        .expect("'//~^ ERROR' anotasyonu") as u64;
    (code, line)
}

#[test]
fn every_fail_fixture_reports_exactly_its_code_on_the_marked_line() {
    let files = fixtures("tests/ui/fail");
    assert_eq!(files.len(), 7, "ADR-0085 ui/fail fixture sayısı");
    let mut bad = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("okunmalı");
        let (code, line) = expectation(&text);
        let env = check_json(file);
        let errs: Vec<(String, u64)> = diagnostics(&env)
            .iter()
            .filter(|d| d["severity"] == "error")
            .map(|d| {
                (
                    d["code"].as_str().unwrap_or_default().to_string(),
                    primary_line(d),
                )
            })
            .collect();
        // Tek tanı: kaskad yok (kol gövdesindeki aynı ad ikinci E1001 vermez).
        if errs != [(code.clone(), line)] {
            bad.push(format!(
                "{}: yalnız {code}@{line} bekleniyor, bulunan {errs:?}",
                file.display()
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn bare_variant_fix_it_writes_the_enum_path() {
    let env = check_json(&root().join("tests/ui/fail/179_pattern_name_bare_variant.volt"));
    let d = diagnostics(&env)
        .iter()
        .find(|d| d["code"] == "E1001")
        .expect("E1001");
    assert!(
        d["help"]
            .as_str()
            .unwrap_or_default()
            .contains("State::Idle"),
        "{d}"
    );
    let fix = d["suggestions"][0]["replacement"]
        .as_str()
        .unwrap_or_default();
    assert_eq!(fix, "State::Idle", "{d}");
}

#[test]
fn signal_name_error_explains_there_are_no_binding_patterns() {
    let env = check_json(&root().join("tests/ui/fail/177_pattern_name_port.volt"));
    let d = &diagnostics(&env)[0];
    let text = d.to_string();
    assert!(
        text.contains("no binding patterns") && text.contains("ADR-0085"),
        "{d}"
    );
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-pn-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn pass_fixture_emits_const_values_as_case_labels() {
    let files = fixtures("tests/ui/pass");
    assert_eq!(files.len(), 1, "ADR-0085 ui/pass fixture sayısı");
    let target = temp_dir("pass");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--target-dir"])
        .arg(&target)
        .arg(&files[0])
        .output()
        .expect("volt çalışmalı");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("warning["), "{stderr}");
    let read = |m: &str| {
        std::fs::read_to_string(target.join("rtl").join(format!("{m}.sv")))
            .unwrap_or_else(|e| panic!("{m}.sv: {e}"))
    };
    let top = read("PatternNames");
    let thr = read("Threshold_4");
    let _ = std::fs::remove_dir_all(&target);
    // Sıralı blok, fn gövdesi (`|` alternatifi) ve enum const'u.
    for part in [
        "8'd10: begin",
        "8'd10: ",
        "8'd1, 8'd2: ",
        "8'd7: ",
        "Mode_Slow: ",
    ] {
        assert!(top.contains(part), "PatternNames.sv'de yok: {part}\n{top}");
    }
    // Generic parametre örnekleme değeriyle.
    assert!(thr.contains("8'd4: "), "{thr}");
    // Ad SV'ye çıplak inmez (SV'de tanımsız ad / yeni sinyal olurdu).
    assert!(
        !top.contains("LIMIT") && !top.contains("DEFAULT_MODE"),
        "{top}"
    );
    // Geniş bildirilmiş const sınananın genişliğinde (Verilator WIDTH).
    assert!(!top.contains("16'd7"), "{top}");
}

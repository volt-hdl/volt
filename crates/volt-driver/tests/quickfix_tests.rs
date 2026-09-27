//! Quick fix sözleşmesi (ADR-0091): `volt check --format=json`'daki
//! `machine-applicable` öneri uygulanınca, yeniden koşulan `volt check`'te
//! o tanı GİDER ve YENİ hata çıkmaz. Fikstürler `tests/quickfix/`
//! (kasıtlı hatalı; çıktı ağı derlemi `tests/fixtures`'ı tarar, bunu
//! değil): başlık `// quickfix: KOD` (kesin düzeltme beklenir) ya da
//! `// no-quickfix: KOD` (tanı var, kesin düzeltme yok).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn check_json(file: &Path) -> Vec<Value> {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["check", "--format=json"])
        .arg(file)
        .env("VOLT_LANG", "en")
        .output()
        .expect("volt çalışmalı");
    let env: Value = serde_json::from_slice(&out.stdout).expect("json");
    env["diagnostics"].as_array().cloned().unwrap_or_default()
}

/// (kod, önem, mesaj) — konumdan bağımsız kimlik: tek satırlık düzeltme
/// sonraki satırları kaydırmaz ama sütunları kaydırabilir.
fn identities(diags: &[Value]) -> Vec<(String, String, String)> {
    let mut ids: Vec<_> = diags
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap_or_default().to_string(),
                d["severity"].as_str().unwrap_or_default().to_string(),
                d["message"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    ids.sort();
    ids
}

fn machine_applicable(d: &Value) -> Vec<&Value> {
    d["suggestions"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|s| s["applicability"] == "machine-applicable")
        .collect()
}

fn header(src: &str) -> (bool, String) {
    let first = src.lines().next().unwrap_or_default();
    if let Some(code) = first.strip_prefix("// quickfix: ") {
        (true, code.trim().to_string())
    } else if let Some(code) = first.strip_prefix("// no-quickfix: ") {
        (false, code.trim().to_string())
    } else {
        panic!("fikstür başlığı yok: {first}")
    }
}

fn fixtures() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(repo("tests/quickfix"))
        .expect("fikstür dizini")
        .map(|e| e.expect("girdi").path())
        .filter(|p| p.extension().is_some_and(|e| e == "volt"))
        .collect();
    files.sort();
    files
}

/// Öneriyi bayt aralığına uygular (JSON `byte` alanları).
fn apply(src: &str, s: &Value) -> String {
    let start = s["span"]["start"]["byte"].as_u64().expect("start") as usize;
    let end = s["span"]["end"]["byte"].as_u64().expect("end") as usize;
    let replacement = s["replacement"].as_str().expect("replacement");
    format!("{}{replacement}{}", &src[..start], &src[end..])
}

fn assert_fix_resolves(file: &Path, code: &str, dir: &Path) {
    let src = std::fs::read_to_string(file).expect("okunmalı");
    let before = check_json(file);
    let target = before
        .iter()
        .find(|d| d["code"] == code)
        .unwrap_or_else(|| panic!("{}: {code} yok", file.display()));
    let fixes = machine_applicable(target);
    assert_eq!(
        fixes.len(),
        1,
        "{}: tek kesin düzeltme: {target}",
        file.display()
    );

    let fixed = dir.join(file.file_name().expect("ad"));
    std::fs::write(&fixed, apply(&src, fixes[0])).expect("yazılmalı");
    let after = check_json(&fixed);

    let count = |ds: &[Value]| ds.iter().filter(|d| d["code"] == code).count();
    assert_eq!(
        count(&after),
        count(&before) - 1,
        "{}: {code} gitmedi: {after:?}",
        file.display()
    );
    // Yeni HATA yasak. Uyarı açığa çıkabilir: ayrıştırma hatası sonraki
    // aşamaları kapılar (ADR-0070); düzeltme kapıyı açınca o aşamaların
    // uyarıları ilk kez görünür (ör. E4008 → W3007).
    let errors = |ds: &[Value]| -> Vec<Value> {
        ds.iter()
            .filter(|d| d["severity"] == "error")
            .cloned()
            .collect()
    };
    let mut remaining = identities(&errors(&before));
    for id in identities(&errors(&after)) {
        let at = remaining
            .iter()
            .position(|r| *r == id)
            .unwrap_or_else(|| panic!("{}: düzeltme YENİ tanı üretti: {id:?}", file.display()));
        remaining.remove(at);
    }
}

#[test]
fn every_machine_applicable_fix_removes_its_diagnostic_without_new_ones() {
    let dir = tempfile_dir("quickfix_apply");
    let mut fixed_codes = Vec::new();
    for file in fixtures() {
        let src = std::fs::read_to_string(&file).expect("okunmalı");
        let (expects_fix, code) = header(&src);
        if expects_fix {
            assert_fix_resolves(&file, &code, &dir);
            fixed_codes.push(code);
        }
    }
    fixed_codes.sort();
    assert_eq!(
        fixed_codes,
        ["E0006", "E0007"],
        "ADR-0091 kesin düzeltme listesi"
    );
}

#[test]
fn uncertain_diagnostics_carry_no_machine_applicable_fix() {
    let mut seen = 0;
    for file in fixtures() {
        let src = std::fs::read_to_string(&file).expect("okunmalı");
        let (expects_fix, code) = header(&src);
        if expects_fix {
            continue;
        }
        seen += 1;
        let diags = check_json(&file);
        let target = diags
            .iter()
            .find(|d| d["code"] == code)
            .unwrap_or_else(|| panic!("{}: {code} yok", file.display()));
        assert!(
            machine_applicable(target).is_empty(),
            "{}: {target}",
            file.display()
        );
    }
    assert!(seen >= 2);
}

/// Did-you-mean önerisi JSON'da `maybe-incorrect` olarak kalır: veri
/// kaybolmaz, yalnız quick fix olmaz.
#[test]
fn a_similar_name_guess_stays_maybe_incorrect_in_json() {
    let diags = check_json(&repo("tests/quickfix/e1001_typo.volt"));
    let e1001 = diags.iter().find(|d| d["code"] == "E1001").expect("E1001");
    assert_eq!(e1001["suggestions"][0]["replacement"], "data");
    assert_eq!(e1001["suggestions"][0]["applicability"], "maybe-incorrect");
}

fn tempfile_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

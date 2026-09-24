//! `tests/ui/pass` fixture'ları derlenir de: `volt build` hatasız SV
//! üretmeli. Anlamsal harness (volt-hir `ui_semantic_tests`) yalnız HIR
//! analizini koşar; emit'in reddettiği bir "pass" fixture'ı orada
//! görünmez (62_extern_domains böyle kaldı — ADR-0071).
//!
//! İstisna yalnız gerekçeli işaretle: `//~ CHECK-ONLY: <neden>` — fixture
//! bilinçli olarak yalnız denetim kuralını gösterir, SV'si anlamsızdır.
//! İşaretli dosya build'den geçerse işaret bayattır ve test düşer.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const MARKER: &str = "//~ CHECK-ONLY";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pass_fixtures() -> Vec<PathBuf> {
    let dir = root().join("tests/ui/pass");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("ui/pass okunmalı")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "volt"))
        .collect();
    files.sort();
    files
}

/// `//~ CHECK-ONLY: <neden>` satırı: `Ok(None)` işaret yok, `Ok(Some)`
/// gerekçe, `Err` gerekçesiz ya da bozuk işaret.
fn check_only_reason(text: &str) -> Result<Option<String>, String> {
    let Some(line) = text.lines().map(str::trim).find(|l| l.starts_with(MARKER)) else {
        return Ok(None);
    };
    let reason = line[MARKER.len()..]
        .strip_prefix(':')
        .map(str::trim)
        .unwrap_or_default();
    if reason.is_empty() {
        return Err(format!(
            "gerekçesiz işaret: '{line}' (biçim: '{MARKER}: <neden>')"
        ));
    }
    Ok(Some(reason.to_string()))
}

fn build_ok(file: &Path, target: &Path) -> Result<(), String> {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--format", "json", "--target-dir"])
        .arg(target)
        .arg(file)
        .output()
        .expect("volt çalışmalı");
    let env: Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("JSON değil ({e}): {}", String::from_utf8_lossy(&out.stderr)))?;
    if env["success"] == true && out.status.success() {
        return Ok(());
    }
    let errors: Vec<String> = env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| format!("{}: {}", d["code"], d["message"]))
        .collect();
    Err(errors.join("; "))
}

#[test]
fn ui_pass_fixtures_build_unless_marked_check_only() {
    let target = std::env::temp_dir().join(format!("volt-uipass-build-{}", std::process::id()));
    let mut bad = Vec::new();
    let (mut built, mut check_only) = (0, 0);
    for file in pass_fixtures() {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&file).expect("okunmalı");
        let marked = match check_only_reason(&text) {
            Ok(m) => m.is_some(),
            Err(e) => {
                bad.push(format!("{name}: {e}"));
                continue;
            }
        };
        let dir = target.join(file.file_stem().unwrap());
        match (build_ok(&file, &dir), marked) {
            (Ok(()), false) => built += 1,
            (Err(_), true) => check_only += 1,
            (Err(e), false) => bad.push(format!("{name}: build edilemiyor: {e}")),
            (Ok(()), true) => bad.push(format!(
                "{name}: CHECK-ONLY işaretli ama build'den geçiyor — işareti kaldırın"
            )),
        }
    }
    let _ = std::fs::remove_dir_all(&target);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    // Sayım, dizin okuması sessizce boş kalırsa testin boşuna geçmesini
    // önler; ui_semantic_tests.rs ve parser_tests.rs sayımlarıyla aynı.
    assert_eq!((built, check_only), (95, 0));
}

#[test]
fn check_only_marker_requires_a_reason() {
    assert_eq!(check_only_reason("module M {}\n"), Ok(None));
    assert_eq!(
        check_only_reason("//~ CHECK-ONLY: yalnız tip kuralı\nmodule M {}\n"),
        Ok(Some("yalnız tip kuralı".to_string()))
    );
    assert!(check_only_reason("//~ CHECK-ONLY\n").is_err());
    assert!(check_only_reason("//~ CHECK-ONLY:   \n").is_err());
    assert!(check_only_reason("//~ CHECK-ONLY neden\n").is_err());
}

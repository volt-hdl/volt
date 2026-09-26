//! match ifadesi ve blok içi `let` uçtan uca (ADR-0083):
//! `tests/ui/fail/*_{implicit_flow,match_expr,block_let}_*` fixture'ları
//! `volt check`'te beklenen kodu beklenen satırda verir (ADR-0070 ortak
//! boru hattı).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const TAGS: [&str; 3] = ["_implicit_flow_", "_match_expr_", "_block_let_"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
        .expect("ui dizini okunmalı")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|e| e == "volt")
                && p.file_name().is_some_and(|n| {
                    let n = n.to_string_lossy();
                    TAGS.iter().any(|t| n.contains(t))
                })
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

/// Hata tanıları: (kod, birincil satır).
fn errors(env: &Value) -> Vec<(String, u64)> {
    env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| {
            let line = d["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true))
                .and_then(|s| s["start"]["line"].as_u64())
                .unwrap_or(0);
            (d["code"].as_str().unwrap_or_default().to_string(), line)
        })
        .collect()
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

// ═══ ui/fail ═════════════════════════════════════════════════════════

#[test]
fn every_fail_fixture_reports_its_code_on_the_marked_line() {
    let files = fixtures("tests/ui/fail");
    assert_eq!(files.len(), 2, "ADR-0083 ui/fail fixture sayısı");
    let mut bad = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("okunmalı");
        let (code, line) = expectation(&text);
        let errs = errors(&check_json(file));
        if !errs.iter().any(|(c, l)| *c == code && *l == line) {
            bad.push(format!(
                "{}: {code}@{line} bekleniyor, bulunan {errs:?}",
                file.display()
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

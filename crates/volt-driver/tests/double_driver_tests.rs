//! Çift sürücü sınıf taraması (ADR-0073): `tests/fixtures/parity/d*.volt`
//! sondaları ADR-0073 tablosunun her satırıdır. İkinci başlık satırı
//!
//! * `// drivers: <birincil satır> <ikincil satır>` — `check` E4001'i iki
//!   sürücünün konumuyla verir, `build` SV üretmeden durur;
//! * `// drivers: ok` — E4001 yok ve üç durumlu hat / `let` başlangıcı
//!   atama sayılmaz (W4001/W4002 yok).
//!
//! Beklenen hata kümesi (`// parity:`) ve LSP eşliği `parity_tests.rs`'te
//! denetlenir; bu dosya konumları ve build'in durduğunu doğrular.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// ADR-0073 tablosu: bu sayıdan az sonda varsa satırlar kaybolmuştur.
const MIN_FIXTURES: usize = 57;

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/parity");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("parite dizini")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|e| e == "volt")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('d'))
        })
        .collect();
    files.sort();
    files
}

fn run_json(args: &[&str], file: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en"])
        .args(args)
        .args(["--format", "json"])
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

fn diagnostics(env: &Value) -> Vec<Value> {
    env["diagnostics"].as_array().cloned().unwrap_or_default()
}

fn span_line(d: &Value, primary: bool) -> Option<u64> {
    d["spans"]
        .as_array()?
        .iter()
        .find(|s| s["primary"] == primary)
        .and_then(|s| s["start"]["line"].as_u64())
}

/// `// drivers:` başlığı: `None` = ok, `Some((birincil, ikincil))`.
fn expected(file: &Path) -> Option<(u64, u64)> {
    let text = std::fs::read_to_string(file).expect("okunmalı");
    let spec = text
        .lines()
        .nth(1)
        .and_then(|l| l.strip_prefix("// drivers:"))
        .unwrap_or_else(|| panic!("{}: '// drivers:' başlığı yok", file.display()))
        .trim()
        .to_string();
    if spec == "ok" {
        return None;
    }
    let lines: Vec<u64> = spec
        .split_whitespace()
        .map(|t| t.parse().expect("satır numarası"))
        .collect();
    assert_eq!(lines.len(), 2, "{}: iki satır bekleniyor", file.display());
    Some((lines[0], lines[1]))
}

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-drivers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn every_double_driver_class_reports_both_locations_and_stops_build() {
    let files = fixtures();
    assert!(
        files.len() >= MIN_FIXTURES,
        "ADR-0073 sondaları: {}",
        files.len()
    );
    let target = temp_dir();
    let target_arg = target.to_string_lossy().into_owned();
    let mut bad = Vec::new();
    for file in &files {
        let check = diagnostics(&run_json(&["check"], file));
        let e4001: Vec<&Value> = check.iter().filter(|d| d["code"] == "E4001").collect();
        match expected(file) {
            Some((primary, secondary)) => {
                let Some(d) = e4001.first() else {
                    bad.push(format!("{}: E4001 yok", file.display()));
                    continue;
                };
                let got = (span_line(d, true), span_line(d, false));
                if got != (Some(primary), Some(secondary)) {
                    bad.push(format!(
                        "{}: konum beklenen ({primary}, {secondary}), gelen {got:?}",
                        file.display()
                    ));
                }
                // Açılımın ürettiği ad (`t_0`) kullanıcıya sızmaz.
                let message = d["message"].as_str().unwrap_or_default();
                if message.split('\'').nth(1).is_some_and(|n| {
                    n.rsplit_once('_')
                        .is_some_and(|(_, k)| k.parse::<u32>().is_ok())
                }) {
                    bad.push(format!("{}: üretilmiş ad sızdı: {message}", file.display()));
                }
                let build = run_json(&["build", "--target-dir", &target_arg], file);
                let build_e4001 = diagnostics(&build).iter().any(|d| d["code"] == "E4001");
                if build["success"] != false || !build_e4001 {
                    bad.push(format!("{}: build E4001 ile durmadı", file.display()));
                }
            }
            None => {
                let noisy: Vec<&str> = check
                    .iter()
                    .filter_map(|d| d["code"].as_str())
                    .filter(|c| ["E4001", "W4001", "W4002"].contains(c))
                    .collect();
                if !noisy.is_empty() {
                    bad.push(format!("{}: beklenmeyen {noisy:?}", file.display()));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

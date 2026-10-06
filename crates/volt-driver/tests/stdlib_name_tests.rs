//! Standard library names on user modules (ADR-0102 §1, issue #92): an
//! `extern module` named like a built-in module (`EdgeDetect`, `SyncFifo`,
//! `Counter`, ...) and the instantiation of a Volt module with such a name
//! are E1016. The built-in used to be taken in their place: the extern's
//! SystemVerilog was ignored and `volt build` could exit 0 with undeclared
//! wires in the output, or fail with an E0003 about the built-in's generic
//! arguments.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const TAG: &str = "_stdlib_name_";

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

fn run_json(cmd: &str, file: &Path, target: Option<&Path>) -> (Option<i32>, Value) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_volt"));
    c.args(["--lang", "en", cmd, "--format", "json"]);
    if let Some(t) = target {
        c.arg("--target-dir").arg(t);
    }
    let out = c.arg(file).output().expect("volt çalışmalı");
    let json = serde_json::from_slice(&out.stdout).expect("JSON");
    (out.status.code(), json)
}

fn errors(json: &Value) -> Vec<(String, u64)> {
    json["diagnostics"]
        .as_array()
        .expect("dizi")
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

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-stdlib-name-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn every_fixture_reports_e1016_alone_on_the_marked_line() {
    let files = fail_fixtures();
    assert_eq!(files.len(), 4, "ADR-0102 §1 ui/fail fixture sayısı");
    let mut bad = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("okunmalı");
        let line = text
            .lines()
            .position(|l| l.trim_start().starts_with("//~^ ERROR"))
            .expect("işaret") as u64;
        for cmd in ["check", "build"] {
            let target = temp_dir(cmd);
            let dir = (cmd == "build").then_some(target.as_path());
            let (code, json) = run_json(cmd, file, dir);
            let errs = errors(&json);
            if code != Some(1) || errs != vec![("E1016".to_string(), line)] {
                bad.push(format!(
                    "{} ({cmd}): exit {code:?}, errors {errs:?}",
                    file.display()
                ));
            }
            let _ = std::fs::remove_dir_all(&target);
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn a_top_module_named_like_the_stdlib_stays_valid() {
    // `volt new`'in şablonu `module Counter` tanımlar ve onu hiçbir modül
    // örneklemez: ad yalnız örnekleme yerinde E1016'dır.
    let dir = temp_dir("top");
    let file = dir.join("counter.volt");
    std::fs::write(
        &file,
        "module Counter {\n    in  clk : clock\n    out n   : u4\n\n    reg n_r : u4 = 0\n    on clk { n_r <= n_r + 1 }\n    n = n_r\n}\n",
    )
    .expect("yazılmalı");
    let (code, json) = run_json("check", &file, None);
    assert_eq!(code, Some(0), "{json}");
    assert!(errors(&json).is_empty(), "{json}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_explanation_names_the_rule_and_the_workaround() {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "explain", "E1016"])
        .output()
        .expect("volt çalışmalı");
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("standard library"), "{text}");
    assert!(text.contains("wrapper"), "{text}");
}

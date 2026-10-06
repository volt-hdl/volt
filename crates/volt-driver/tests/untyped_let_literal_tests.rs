//! Tipsiz literal `let` (#80, type-inference.md §5 W2012): `let k = 100`
//! W2012 ile `i32` varsayılır ve öyle kullanılır. SV üretimi bu kuralı
//! uygulamıyordu: aynı satır hem W2012 ("i32 assumed") hem E2005 ("cannot
//! determine the width") veriyordu. Tek, doğru tanı W2012'dir; `i32`'ye
//! sığmayan literal sessizce kesilmez, açık tipli `let` gibi E2010'dur.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-untyped-let-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn design(value: &str) -> String {
    format!("module Add {{\n    in  x : i32\n    out y : i32\n\n    let k = {value}\n    y = x + k\n}}\n")
}

/// `(çıkış kodu, tanı kodları, üretilen SV)`.
fn build(tag: &str, value: &str) -> (Option<i32>, Vec<String>, Option<String>) {
    let dir = temp_dir(tag);
    let file = dir.join("add.volt");
    std::fs::write(&file, design(value)).expect("yazılmalı");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--format", "json", "--target-dir"])
        .arg(dir.join("build"))
        .arg(&file)
        .output()
        .expect("volt çalışmalı");
    let json: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    let codes = json["diagnostics"]
        .as_array()
        .expect("dizi")
        .iter()
        .filter_map(|d| d["code"].as_str().map(str::to_string))
        .collect();
    let sv = std::fs::read_to_string(dir.join("build/rtl/Add.sv")).ok();
    let _ = std::fs::remove_dir_all(&dir);
    (out.status.code(), codes, sv)
}

fn wire_line(sv: &str) -> &str {
    sv.lines()
        .find(|l| l.contains(" k ") || l.contains(" k="))
        .unwrap_or_default()
}

#[test]
fn an_unsuffixed_literal_let_is_w2012_alone_and_an_i32_wire() {
    for (tag, value, literal) in [
        ("dec", "100", "32'sd100"),
        ("big", "100000", "32'sd100000"),
        ("hex", "0x10", "32'h10"),
    ] {
        let (code, codes, sv) = build(tag, value);
        assert_eq!(codes, vec!["W2012"], "let k = {value}");
        assert_eq!(code, Some(0), "let k = {value}");
        let sv = sv.expect("SV yazılmalı");
        let line = wire_line(&sv);
        assert!(
            line.contains("signed [31:0] k") && line.contains(literal),
            "let k = {value}: {line}\n{sv}"
        );
    }
}

#[test]
fn a_negative_literal_let_is_an_i32_wire() {
    let (code, codes, sv) = build("neg", "-5");
    assert_eq!(codes, vec!["W2012"]);
    assert_eq!(code, Some(0));
    let sv = sv.expect("SV yazılmalı");
    assert!(wire_line(&sv).contains("signed [31:0] k"), "{sv}");
}

#[test]
fn a_literal_that_does_not_fit_i32_is_e2010_alone() {
    let (code, codes, sv) = build("wide", "5000000000");
    assert_eq!(codes, vec!["E2010"]);
    assert_eq!(code, Some(1));
    assert!(sv.is_none());
}

#[test]
fn the_w2012_fix_leaves_no_diagnostic() {
    // Öneri (` : i32`) uygulanmış biçim temiz derlenir.
    let dir = temp_dir("fixed");
    let file = dir.join("add.volt");
    std::fs::write(
        &file,
        design("100").replace("let k = 100", "let k : i32 = 100"),
    )
    .expect("yazılmalı");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(&file)
        .output()
        .expect("volt çalışmalı");
    let json: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert_eq!(out.status.code(), Some(0), "{json}");
    assert!(
        json["diagnostics"].as_array().expect("dizi").is_empty(),
        "{json}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

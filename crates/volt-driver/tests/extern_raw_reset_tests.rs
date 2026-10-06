//! W3011 (ADR-0103, #93): a raw reset port that Volt synchronizes for the
//! module's own registers also goes, raw, to an extern instance. The
//! warning changes nothing in the output: the build succeeds and the
//! SystemVerilog keeps the synchronizer for the registers and the raw
//! reset on the extern.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ui/fail/219_extern_raw_reset.volt")
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "volt-extern-raw-reset-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn build_warns_once_and_keeps_the_output() {
    let target = temp_dir("build");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--format", "json", "--target-dir"])
        .arg(&target)
        .arg(fixture())
        .output()
        .expect("volt çalışmalı");
    let json: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert_eq!(out.status.code(), Some(0), "{json}");
    let codes: Vec<&str> = json["diagnostics"]
        .as_array()
        .expect("dizi")
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert_eq!(codes, vec!["W3011"], "{json}");
    let sv = std::fs::read_to_string(target.join("rtl/ExternRawReset.sv")).expect("SV yazılmalı");
    assert!(
        sv.contains("rst_sync_clk_stage1"),
        "register'lar senkronize reset kullanır:\n{sv}"
    );
    assert!(sv.contains(".rst  (rst)"), "extern ham reset'i alır:\n{sv}");
    let _ = std::fs::remove_dir_all(&target);
}

//! Reset'in örneğe yolu (ADR-0124, #340), `volt` ikilisi üzerinden.
//!
//! - #340'ın tasarımı (ham reset `sync()`'ten geçip çocuğa) E2003 alır;
//!   Volt alt modülünde makine önerisi var, extern'de yok ve not W3011'i
//!   ile iki yolu anlatır. Tanı metni ADR numarası ve spec yolu taşımaz.
//! - Doğru yol: reset portu yazılmayan çocuk, simülasyonda reset değerinden
//!   başlar (Verilator gerekir; araç yoksa atlanır, `VOLT_REQUIRE_TOOLS`
//!   ile zorunluysa düşer, ADR-0079).
//! - Extern'e ham reset: yalnız W3011, değişmedi.

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tools::Tool;

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn volt() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
    cmd.env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env_remove("VOLT_MANIFEST_DIR");
    cmd
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-reset-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// `volt check --format=json` tanıları (dosya başına zarf).
fn check_json(file: &Path) -> Vec<Value> {
    let out = volt()
        .args(["check", "--format=json"])
        .arg(file)
        .output()
        .expect("volt çalışmalı");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    serde_json::Deserializer::from_str(&stdout)
        .into_iter::<Value>()
        .map(|v| v.expect("json zarfı"))
        .flat_map(|env| env["diagnostics"].as_array().cloned().unwrap_or_default())
        .collect()
}

fn codes(diags: &[Value]) -> Vec<String> {
    let mut c: Vec<String> = diags
        .iter()
        .map(|d| d["code"].as_str().unwrap_or_default().to_string())
        .collect();
    c.sort();
    c
}

/// Tanının okura dönük bütün metni: ileti, etiketler, yardım, notlar.
fn reader_text(d: &Value) -> String {
    // Konum alanları dosya yolu taşır; yalnız okunan metin denetlenir.
    let mut t = String::new();
    for key in ["message", "help"] {
        t.push_str(d[key].as_str().unwrap_or_default());
        t.push('\n');
    }
    for s in d["spans"].as_array().into_iter().flatten() {
        t.push_str(s["label"].as_str().unwrap_or_default());
        t.push('\n');
    }
    for n in d["notes"].as_array().into_iter().flatten() {
        t.push_str(n["text"].as_str().unwrap_or_default());
        t.push('\n');
    }
    t
}

#[test]
fn issue_340_design_is_e2003_with_a_fix_for_the_volt_child() {
    let diags = check_json(&repo("tests/real_bugs/340_reset_through_sync.volt"));
    assert_eq!(codes(&diags), ["E2003", "E2003"], "{diags:#?}");
    let fixes: Vec<&Value> = diags
        .iter()
        .flat_map(|d| d["suggestions"].as_array().into_iter().flatten())
        .collect();
    assert_eq!(fixes.len(), 1, "{diags:#?}");
    assert_eq!(fixes[0]["applicability"], "machine-applicable");
    for d in &diags {
        let t = reader_text(d);
        assert!(!t.contains("ADR"), "{t}");
        assert!(!t.contains(".md"), "{t}");
        assert!(!t.contains("sv-mapping"), "{t}");
    }
}

#[test]
fn a_reset_through_a_let_to_an_extern_has_no_fix_and_names_both_ways() {
    let dir = temp_dir("extern");
    let file = dir.join("top.volt");
    std::fs::write(
        &file,
        "extern module Latch {
    in  clk  : clock
    in  rst  : reset(sync, active_high)
    out seen : bool
}

module Top {
    in  clk  : clock
    in  rst  : reset(sync, active_high)
    out seen : bool
    out n    : u4

    reg n_r : u4 = 0
    on clk {
        n_r <= n_r + 1
    }
    n = n_r
    let rst_s = rst
    let l = Latch { clk: clk, rst: rst_s }
    seen = l.seen
}
",
    )
    .expect("yazılmalı");
    let diags = check_json(&file);
    assert_eq!(codes(&diags), ["E2003"], "{diags:#?}");
    let d = &diags[0];
    assert!(
        d["suggestions"].as_array().is_none_or(|a| a.is_empty()),
        "{d:#}"
    );
    let t = reader_text(d);
    // Neden bugün yol yok, ham bağlamanın bedeli (W3011) ve öteki yol.
    assert!(t.contains("cannot give an extern module"), "{t}");
    assert!(t.contains("W3011"), "{t}");
    assert!(t.contains("synchronize inside the extern"), "{t}");
    assert!(t.contains("rst: rst"), "{t}");
    assert!(!t.contains("ADR"), "{t}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_raw_reset_to_an_extern_is_still_only_w3011() {
    let diags = check_json(&repo("tests/ui/fail/219_extern_raw_reset.volt"));
    assert_eq!(codes(&diags), ["W3011"], "{diags:#?}");
}

#[test]
fn a_child_without_a_reset_port_starts_from_its_reset_value_in_simulation() {
    if tools::require(Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    let design = std::fs::read_to_string(repo("tests/ui/pass/158_reset_child_automatic_port.volt"))
        .expect("fikstür");
    std::fs::write(dir.join("top.volt"), design).expect("yazılmalı");
    // Değerler elle (ADR-0033): reset() dönünce register reset değerinde
    // (5); her saat +1. İkinci reset() yine 5'e döndürür. #340'ın sync()
    // biçiminde çocuk reset'i hiç görmüyor, sayaç güç açılışından sayıyordu.
    std::fs::write(
        dir.join("top_test.volt"),
        "test \"the child is reset through its automatic port\" {
    let dut = AutomaticResetPort { };
    reset();
    assert_eq(dut.v, 5);
    step(3);
    assert_eq(dut.v, 8);
    reset();
    assert_eq(dut.v, 5);
}
",
    )
    .expect("yazılmalı");
    let out = volt()
        .current_dir(&dir)
        .args(["test", "top_test.volt"])
        .output()
        .expect("volt test çalışmalı");
    let log = text(&out);
    assert_eq!(out.status.code(), Some(0), "{log}");
    assert!(log.contains("1 passed; 0 failed"), "{log}");
    let _ = std::fs::remove_dir_all(&dir);
}

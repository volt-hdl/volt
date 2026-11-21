//! Reset veri olarak okunamaz (ADR-0065 R3, #356), `volt` ikilisi
//! üzerinden.
//!
//! - #356'nın tasarımı (`match rst` → `sync()`) E2003 alır; makine önerisi
//!   yok, tanı metni ADR numarası ve spec yolu taşımaz.
//! - Kontrol tablosunun sürücüde reddedilen satırları (`concat`, `zext`:
//!   ifade içinde çağrı, E0003) değişmedi.
//! - Help'in iki yolu simülasyonda: register'ın reset değeri reset'ten
//!   sonra, ayrı `bool` giriş portu yazılımla tetiklenen yumuşak reset
//!   olarak (Verilator gerekir; araç yoksa atlanır, `VOLT_REQUIRE_TOOLS`
//!   ile zorunluysa düşer, ADR-0079).

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
    let dir = std::env::temp_dir().join(format!("volt-reset-data-{tag}-{}", std::process::id()));
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
fn issue_356_design_is_e2003_without_a_fix() {
    let diags = check_json(&repo("tests/real_bugs/356_reset_read_as_data.volt"));
    assert_eq!(codes(&diags), ["E2003"], "{diags:#?}");
    let d = &diags[0];
    assert!(
        d["suggestions"].as_array().is_none_or(Vec::is_empty),
        "makine önerisi olmamalı: {d:#?}"
    );
    let t = reader_text(d);
    for part in [
        "a reset cannot be tested by match",
        "a reset is not a value",
    ] {
        assert!(t.contains(part), "{part:?} yok:\n{t}");
    }
    assert!(!t.contains("ADR"), "{t}");
    assert!(!t.contains(".md"), "{t}");
}

#[test]
fn concat_and_zext_of_a_reset_are_still_e0003() {
    let dir = temp_dir("calls");
    for e in ["concat(rst, rst)", "concat(rst, true)", "zext(rst)"] {
        let f = dir.join("m.volt");
        std::fs::write(
            &f,
            format!(
                "module M {{
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    let t = {e}
    y = sync(t, clk)
}}
"
            ),
        )
        .expect("yazılmalı");
        let c = codes(&check_json(&f));
        assert!(c.iter().any(|x| x == "E0003"), "{e}: {c:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_helps_two_ways_behave_in_simulation() {
    if tools::require(Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    // Reset'e bağlı davranış register'ın reset değerindedir (5); yazılımla
    // tetiklenen yumuşak reset ayrı bir bool giriştir (`soft`).
    std::fs::write(
        dir.join("top.volt"),
        "module SoftReset {
    in  clk  : clock
    in  rst  : reset(sync, active_high)
    in  soft : bool
    out q    : u8

    reg r : u8 = 5
    on clk {
        if soft {
            r <= 5
        } else {
            r <= r + 1
        }
    }
    q = r
}
",
    )
    .expect("yazılmalı");
    // Değerler elle (ADR-0033): reset() dönünce r reset değerinde (5); her
    // saat +1. soft bir kenar boyunca 1 iken r <= 5; sonra yine +1. İkinci
    // reset() 5'e döndürür.
    std::fs::write(
        dir.join("top_test.volt"),
        "test \"reset value and a soft reset input\" {
    let dut = SoftReset { };
    dut.soft = false;
    reset();
    assert_eq(dut.q, 5);
    step(3);
    assert_eq(dut.q, 8);
    dut.soft = true;
    step(1);
    assert_eq(dut.q, 5);
    dut.soft = false;
    step(2);
    assert_eq(dut.q, 7);
    reset();
    assert_eq(dut.q, 5);
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

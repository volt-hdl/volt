//! Bildirilen `Delayed<T, N>` gerçek gecikmeyle karşılaştırılır (ADR-0126,
//! #354), `volt` ikilisi üzerinden.
//!
//! - #354'ün tasarımı (karışım yok, `let`'in bildirimi yanlış) E5010 alır;
//!   tanı ne olduğunu, nedenini ve iki yolu söyler, `delay<K>` önermez
//!   (#232: bugün register eklemez) ve makine önerisi taşımaz.
//! - Gerçekten hizalayan tasarım (aşama register'ı) simülasyonda hâlâ aynı
//!   yaştaki iki değeri toplar (Verilator gerekir; araç yoksa atlanır,
//!   `VOLT_REQUIRE_TOOLS` ile zorunluysa düşer, ADR-0079).

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
    let dir = std::env::temp_dir().join(format!("volt-delayed-{tag}-{}", std::process::id()));
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
fn check_json(file: &Path, lang: &str) -> Vec<Value> {
    let out = volt()
        .env("VOLT_LANG", lang)
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
fn issue_354_design_is_e5010_at_the_declaration() {
    let file = repo("tests/real_bugs/354_delayed_let_wrong_count.volt");
    for lang in ["en", "tr"] {
        let diags = check_json(&file, lang);
        assert_eq!(codes(&diags), ["E5010"], "{lang}: {diags:#?}");
        let d = &diags[0];
        // Birincil konum bildirimdir (`Delayed<u32, 1>`, satır 18).
        let primary = d["spans"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|s| s["primary"] == true)
            .expect("birincil konum");
        assert_eq!(primary["start"]["line"], 18, "{lang}: {d:#}");
        // Makine önerisi yok: N'i değiştirmek aşağıdaki karışımı geri
        // getirebilir (gidiş-dönüş testi geçmez).
        assert!(
            d["suggestions"].as_array().is_none_or(|a| a.is_empty()),
            "{lang}: {d:#}"
        );
        let t = reader_text(d);
        // Ne oldu: iki sayı; neden: gerekçe notu; ne yapılabilir: help.
        assert!(t.contains('1') && t.contains('2'), "{lang}: {t}");
        assert!(
            d["notes"]
                .as_array()
                .is_some_and(|n| n.iter().any(|n| n["kind"] == "reason")),
            "{lang}: {d:#}"
        );
        assert!(!d["help"].as_str().unwrap_or_default().is_empty(), "{lang}");
        // #232 çözülmeden delay<K> önerilmez; ADR numarası ve spec yolu yok.
        assert!(!t.contains("delay<"), "{lang}: {t}");
        assert!(!t.contains("ADR"), "{lang}: {t}");
        assert!(!t.contains(".md"), "{lang}: {t}");
    }
}

#[test]
fn a_mix_under_a_declaration_still_compiles() {
    // #354, tasarım 1: ileri besleme kaçışı bu düzeltmede değişmez.
    let dir = temp_dir("mix");
    let file = dir.join("top.volt");
    std::fs::write(
        &file,
        "@strict_timing
module P {
    in  clk : clock
    in  x   : u32
    out y   : u32
    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    on clk { a <= x  b <= a }
    let s : Delayed<u32, 2> = a + b
    y = s
}
",
    )
    .expect("yazılmalı");
    let diags = check_json(&file, "en");
    assert!(codes(&diags).is_empty(), "{diags:#?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn aligned_stage_registers_still_add_values_of_the_same_age() {
    if tools::require(Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    // #354'ün kontrol tasarımı: `c` gerçek bir aşama register'ı, `b` ile
    // aynı yaşta (2 çevrim).
    std::fs::write(
        dir.join("top.volt"),
        "@strict_timing
module P {
    in  clk : clock
    in  x   : u32
    out y   : u32
    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    reg c : Delayed<u32, 2> = 0
    on clk { a <= x  b <= a  c <= a }
    y = c + b
}
",
    )
    .expect("yazılmalı");
    // Değer elle (ADR-0033): üç saatten sonra a = 30, b = c = 20 (x iki
    // çevrim önce); y = 20 + 20 = 40. #354'ün yanlış bildirimli tasarımı
    // 50 (= 30 + 20, iki ayrı çevrimin değeri) veriyordu.
    std::fs::write(
        dir.join("top_test.volt"),
        "test \"aligned\" {
    let dut = P { };
    dut.x = 10;
    step(1);
    dut.x = 20;
    step(1);
    dut.x = 30;
    step(1);
    assert_eq(dut.y, 40);
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

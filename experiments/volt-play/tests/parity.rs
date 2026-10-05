//! Oyun alanı çekirdeği ile gerçek `volt` aynı sonucu veriyor mu?
//!
//! Her örnek geçici bir dizine `main.volt` olarak yazılır, orada
//! `volt build --single-file --format json` koşulur; tanı kodları ve
//! üretilen SV, bellekteki `volt_play::compile` sonucuyla karşılaştırılır.
//! `volt` ikilisi: `VOLT_BIN` ya da depo kökündeki `target/debug/volt`.

use std::path::{Path, PathBuf};
use std::process::Command;

use volt_diagnostics::Lang;

fn volt_bin() -> PathBuf {
    std::env::var_os("VOLT_BIN").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/volt"),
        PathBuf::from,
    )
}

fn example(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples_src")
        .join(format!("{name}.volt"));
    std::fs::read_to_string(path).expect("örnek okunmalı")
}

/// Gerçek `volt`: (tanı kodları, SV).
fn real_volt(name: &str, src: &str) -> (Vec<String>, Option<String>) {
    let dir = std::env::temp_dir().join(format!("volt-play-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.volt"), src).unwrap();
    let out = Command::new(volt_bin())
        .current_dir(&dir)
        .env("VOLT_MANIFEST_DIR", &dir)
        .args(["build", "--single-file", "--format", "json", "--lang", "en"])
        .args(["--target-dir", "build", "main.volt"])
        .output()
        .expect("volt koşulmalı (cargo build -p volt-driver)");
    let env: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON zarfı");
    let codes = env["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap().to_string())
        .collect();
    let sv = std::fs::read_to_string(dir.join("build/rtl/main.sv")).ok();
    let _ = std::fs::remove_dir_all(&dir);
    (codes, sv)
}

fn check(name: &str) {
    check_source(name, &example(name));
    let play = volt_play::compile(&example(name), Lang::En);
    if play.errors() == 0 {
        assert!(
            play.sv.as_deref().is_some_and(|sv| sv.contains("module ")),
            "{name}: SV boş"
        );
    }
}

fn check_source(name: &str, src: &str) {
    let src = src.to_string();
    let play = volt_play::compile(&src, Lang::En);
    let json = play.to_json();
    let codes: Vec<String> = json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap().to_string())
        .collect();
    let (real_codes, real_sv) = real_volt(name, &src);
    assert_eq!(codes, real_codes, "{name}: tanı kodları");
    assert_eq!(play.sv, real_sv, "{name}: SV");
}

#[test]
fn blinky_matches_volt_build() {
    check("blinky");
}

#[test]
fn counter_matches_volt_build() {
    check("counter");
}

#[test]
fn mmio_blinker_matches_volt_build() {
    check("mmio_blinker");
}

#[test]
fn cdc_error_matches_volt_build() {
    check("cdc_error");
    let play = volt_play::compile(&example("cdc_error"), Lang::En);
    assert_eq!(play.errors(), 1);
    assert!(play.sv.is_none());
    let human = &play.to_json()["rendered"][0];
    assert!(human.as_str().unwrap().contains("error[E3001]"), "{human}");
}

#[test]
fn turkish_diagnostics() {
    let play = volt_play::compile(&example("cdc_error"), Lang::Tr);
    let text = play.to_json()["rendered"][0].as_str().unwrap().to_string();
    volt_diagnostics::set_lang(Lang::En);
    assert!(text.contains("neden:"), "{text}");
}

#[test]
fn file_without_modules_has_no_sv() {
    check_source("empty", "");
    check_source("library", "pub fn twice(x: u8) -> u8 {\n    x + x\n}\n");
    assert!(volt_play::compile("", Lang::En).sv.is_none());
}

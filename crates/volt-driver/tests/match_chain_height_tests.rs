//! İç konumdaki `match` zincirinin yüksekliği (#336).
//!
//! Kural (sv-mapping.md §19.1, ADR-0083 Karar 10.3): iç konumdaki
//! zincirin yüksekliği kol sayısıdır; 256'yı aşarsa E0018. Sınır: 256
//! kollu match geçerlidir, E0018 257 koldan başlar. Yaprak kol gövdesi kat
//! eklemez; gövdedeki işleçler eklenir; iç içe match'te yol üzerindeki kol
//! sayıları toplanır (spec bunu söylemiyor: bakımcı kararı, #336).
//!
//! Düzeltmeden önce ölçü yaprak kol gövdesine bir kat ekliyordu (N kol →
//! N + 1): 256 kollu match E0018 alıyordu.
//!
//! Beklenen değerler kuraldan elle yazıldı (her testin yorumunda), derleyici
//! çıktısından değil. Verilator yoksa davranış testi atlanır;
//! `VOLT_REQUIRE_TOOLS=verilator` ile (CI'ın araçlı işi) yokluk testi
//! düşürür (ADR-0079 §3).

mod tools;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// `arms` kollu match'in kolları: `arms - 1` sabit kol (`k => gövde(k)`,
/// k = 0 … arms - 2) ve joker `_ => last`.
fn arms_of(arms: usize, body: impl Fn(usize) -> String, last: &str) -> String {
    let mut text = String::new();
    for k in 0..arms - 1 {
        text.push_str(&format!("{k} => {}, ", body(k)));
    }
    text.push_str(&format!("_ => {last}"));
    text
}

/// `expr` iç konumdadır: `^ a`'nın operandı, tüm sağ taraf değil (§19.1).
fn module_of(expr: &str) -> String {
    format!(
        "module H {{\n    in  s : u16\n    in  a : u8\n    out y : u8\n    \
         y = ({expr}) ^ a\n}}\n"
    )
}

/// `arms` kollu iç konum match'i, yaprak gövdeli: `k => 254 - k` (256'dan
/// büyük kol sayısında değer 8 bite sığsın diye `k % 255`), `_ => 255`.
fn design(arms: usize) -> String {
    let arms = arms_of(arms, |k| (254 - (k % 255)).to_string(), "255");
    module_of(&format!("match s {{ {arms} }}"))
}

/// Gövdeleri işleç olan match: `k => a ^ k`, `_ => a`. Her gövde zincire
/// bir kat ekler (`^`).
fn operator_bodies(arms: usize) -> String {
    let arms = arms_of(arms, |k| format!("a ^ {}", k % 200), "a");
    module_of(&format!("match s {{ {arms} }}"))
}

/// `outer` kollu match'in joker kolu `inner` kollu bir match'tir (iki
/// match de yaprak gövdeli).
fn nested(outer: usize, inner: usize) -> String {
    let inner = arms_of(inner, |k| (k % 200).to_string(), "7");
    let outer = arms_of(
        outer,
        |k| (k % 200).to_string(),
        &format!("match s {{ {inner} }}"),
    );
    module_of(&format!("match s {{ {outer} }}"))
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-match-height-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn volt(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("volt çalışmalı")
}

fn text_of(out: &std::process::Output) -> String {
    format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// `volt check --format json`: tanıların kodları.
fn check_codes(tag: &str, src: &str) -> Vec<String> {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("case.volt"), src).expect("yaz");
    let out = volt(&dir, &["check", "--format", "json", "case.volt"]);
    let _ = std::fs::remove_dir_all(&dir);
    let json: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("JSON değil ({e}): {}", String::from_utf8_lossy(&out.stderr)));
    json["diagnostics"]
        .as_array()
        .expect("diagnostics dizisi")
        .iter()
        .map(|d| d["code"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// 256 kol: zincirin yüksekliği 256'dır, 256'yı aşmaz — tanı yok.
#[test]
fn a_match_of_256_arms_in_an_inner_position_is_accepted() {
    assert_eq!(check_codes("256", &design(256)), Vec::<String>::new());
}

/// 257 kol: yükseklik 257 > 256 — tek tanı E0018.
#[test]
fn a_match_of_257_arms_in_an_inner_position_is_e0018() {
    assert_eq!(check_codes("257", &design(257)), ["E0018"]);
}

/// Gövdedeki işleç zincirin yüksekliğine eklenir: 255 kol + `^` = 256,
/// kabul; 256 kol + `^` = 257 > 256, E0018 (zincir 256 kollu olsa da).
#[test]
fn an_operator_in_the_arm_bodies_adds_its_level() {
    assert_eq!(
        check_codes("op255", &operator_bodies(255)),
        Vec::<String>::new()
    );
    assert_eq!(check_codes("op256", &operator_bodies(256)), ["E0018"]);
}

/// İç içe match'te yol üzerindeki kol sayıları toplanır: 128 + 128 = 256,
/// kabul; 128 + 129 = 257 > 256, tek tanı E0018 (yalnız en dıştaki iç
/// match denetlenir).
#[test]
fn nested_matches_add_their_arm_counts() {
    assert_eq!(check_codes("n256", &nested(128, 128)), Vec::<String>::new());
    assert_eq!(check_codes("n257", &nested(128, 129)), ["E0018"]);
}

/// Beklenen değerler (`a` = 15 = 0x0F, `y = v ^ a`, kollar `k => 254 - k`):
/// - s = 0 (ilk kol): v = 254 = 0xFE, y = 0xF1 = 241;
/// - s = 127: v = 127 = 0x7F, y = 0x70 = 112;
/// - s = 254 (son sabit kol): v = 0, y = 15;
/// - s = 255 ve s = 1000 (joker): v = 255 = 0xFF, y = 0xF0 = 240.
const TESTS: &str = r#"
test "literal arms: the first, a middle one and the last" {
    let dut = H { };
    dut.a = 15;
    dut.s = 0;
    step(1);
    assert_eq(dut.y, 241);
    dut.s = 127;
    step(1);
    assert_eq(dut.y, 112);
    dut.s = 254;
    step(1);
    assert_eq(dut.y, 15);
}

test "the wildcard takes every other value" {
    let dut = H { };
    dut.a = 15;
    dut.s = 255;
    step(1);
    assert_eq(dut.y, 240);
    dut.s = 1000;
    step(1);
    assert_eq(dut.y, 240);
}
"#;

/// 256 kollu tasarımın SV'si Verilator `--lint-only -Wall`'dan bulgusuz
/// geçer ve simülasyonda kuraldan elle hesaplanan değerleri verir.
#[test]
fn a_match_of_256_arms_lints_clean_and_selects_by_the_spec() {
    let Some(verilator) = tools::require(tools::Tool::Verilator) else {
        return;
    };
    let dir = temp_dir("sim");
    std::fs::write(dir.join("case.volt"), format!("{}{TESTS}", design(256))).expect("yaz");

    let out = volt(&dir, &["build", "--target-dir", "out", "case.volt"]);
    assert!(out.status.success(), "{}", text_of(&out));
    let sv = std::fs::read_to_string(dir.join("out/rtl/H.sv")).expect("H.sv");
    // İç konum üçlü zincirdir (ADR-0083 Karar 10.2): 255 sabit kol 255
    // `?` verir; test zincirin kendisini sınasın, `case`'i değil.
    assert!(!sv.contains("case ("), "{sv}");
    assert_eq!(sv.matches(" ? ").count(), 255, "{sv}");

    let lint = Command::new(&verilator)
        .args(["--lint-only", "-Wall", "--top-module", "H", "H.sv"])
        .current_dir(dir.join("out/rtl"))
        .output()
        .expect("verilator çalışmalı");
    let lint_text = text_of(&lint);
    // 5.05x `--lint-only`'de de "Verilation Report" özeti basar; ölçüt
    // uyarı/hata satırı olmaması.
    let findings = lint_text.lines().filter(|l| l.starts_with('%')).count();
    assert!(lint.status.success() && findings == 0, "{lint_text}");

    let out = volt(&dir, &["test", "case.volt", "--target-dir", "build"]);
    let text = text_of(&out);
    // CI günlüğünde testin Verilator'da koştuğu görünsün (atlanan test de
    // "ok" yazar).
    eprintln!("--- volt test (match_chain_height_tests::sim)\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(text.contains("2 passed; 0 failed"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

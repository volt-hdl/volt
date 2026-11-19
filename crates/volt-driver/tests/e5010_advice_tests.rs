//! E5010'un tavsiyesi davranış olarak (ADR-0127, #232; ADR-0125 §4).
//!
//! - `volt explain E5010`'un ÖRNEK bloğu tam bir modüldür ve yalnız
//!   E5010 verir; ÇÖZÜM bloğu tanısız derlenir (iki dilde).
//! - Help'teki kod (aşama register'ı) ÖRNEK'e uygulanınca E5010 gider ve
//!   yeni tanı çıkmaz (iki dilde; kod parçaları help'ten okunur).
//! - Uygulanmış tasarım ve ÇÖZÜM, Verilator'da kontrol tasarımının
//!   değerini verir: iki değer de gerçekten aynı yaştadır.
//!
//! Beklenen değer elle hesaplandı: `x` 1.–4. kenardan önce 10, 20, 30,
//! 40; 4. kenardan sonra `a` = 40 (1 çevrim), `b` = 30 (2 çevrim),
//! `a_d2` = 3. kenardan sonraki `a` = 30 (2 çevrim). Hizalı toplam
//! 30 + 30 = 60. Eski tavsiye (`delay<1>(a) + b`) register eklemediği
//! için 40 + 30 = 70 verir (`tests/real_bugs/232_*`).
//!
//! Verilator yoksa simülasyon testi atlanır; `VOLT_REQUIRE_TOOLS` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

use volt_diagnostics::explain::explanation;
use volt_diagnostics::{ErrorCode, Lang};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-e5010-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt check --format json`: tanıların (kod, help) listesi.
fn check(tag: &str, lang: &str, src: &str) -> Vec<(String, String)> {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("p.volt"), src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["check", "p.volt", "--format", "json", "--lang", lang])
        .output()
        .expect("volt check");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "JSON değil ({e}): {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
    v["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap_or_default().to_string(),
                d["help"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn lang_of(lang: &str) -> Lang {
    if lang == "tr" {
        Lang::Tr
    } else {
        Lang::En
    }
}

#[test]
fn explain_example_gives_e5010_and_solution_compiles_clean() {
    for lang in ["en", "tr"] {
        let ex = explanation(lang_of(lang), ErrorCode::E5010);
        let codes: Vec<String> = check(&format!("ex-{lang}"), lang, ex.example)
            .into_iter()
            .map(|(c, _)| c)
            .collect();
        assert_eq!(codes, ["E5010"], "{lang} ÖRNEK:\n{}", ex.example);
        let fix = check(&format!("fix-{lang}"), lang, ex.fix);
        assert!(
            fix.is_empty(),
            "{lang} ÇÖZÜM tanı veriyor: {fix:?}\n{}",
            ex.fix
        );
    }
}

/// Help'teki ``…`` parçaları.
fn code_parts(help: &str) -> Vec<&str> {
    help.split('`').skip(1).step_by(2).collect()
}

/// ÖRNEK'e help'in kodunu uygular: `reg` bildirimi `on` bloğundan önce,
/// yükleme `on clk` bloğunun içine, genç değerin yerine son register.
fn apply_help(example: &str, help: &str) -> String {
    let parts = code_parts(help);
    let decl = parts
        .iter()
        .find(|p| p.starts_with("reg "))
        .unwrap_or_else(|| panic!("help'te reg bildirimi yok: {help}"));
    let load = parts
        .iter()
        .find(|p| p.contains(" <= "))
        .unwrap_or_else(|| panic!("help'te yükleme yok: {help}"));
    let (reg, value) = load.split_once(" <= ").expect("yükleme");
    let on = "    on clk { a <= x  b <= a }";
    assert!(example.contains(on), "{example}");
    let operand = format!("sum = {value} + b");
    assert!(example.contains(&operand), "{example}\n{help}");
    example
        .replace(
            on,
            &format!("    {decl}\n    on clk {{ a <= x  b <= a  {load} }}"),
        )
        .replace(&operand, &format!("sum = {reg} + b"))
}

#[test]
fn the_help_code_applied_to_the_example_removes_e5010() {
    for lang in ["en", "tr"] {
        let ex = explanation(lang_of(lang), ErrorCode::E5010);
        let diags = check(&format!("help-{lang}"), lang, ex.example);
        assert_eq!(diags.len(), 1, "{diags:?}");
        let applied = apply_help(ex.example, &diags[0].1);
        let after = check(&format!("applied-{lang}"), lang, &applied);
        assert!(after.is_empty(), "{lang}: {after:?}\n{applied}");
    }
}

const ALIGNED_TEST: &str = r#"
test "both values are of the same age" {
    let dut = P { };
    dut.x = 10;
    step(1);
    dut.x = 20;
    step(1);
    dut.x = 30;
    step(1);
    dut.x = 40;
    step(1);
    // a_d2 = 30 (x3), b = 30 (x3): 60. Without the register: 40 + 30 = 70.
    assert_eq(dut.sum, 60);
}
"#;

#[test]
fn the_help_code_and_the_solution_align_the_values_in_verilator() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let ex = explanation(Lang::En, ErrorCode::E5010);
    let diags = check("sim-help", "en", ex.example);
    assert_eq!(diags.len(), 1, "{diags:?}");
    let applied = apply_help(ex.example, &diags[0].1);
    for (tag, design) in [("applied", applied.as_str()), ("solution", ex.fix)] {
        let dir = temp_dir(&format!("sim-{tag}"));
        std::fs::write(dir.join("p.volt"), format!("{design}\n{ALIGNED_TEST}")).expect("yaz");
        let out = Command::new(env!("CARGO_BIN_EXE_volt"))
            .env("VOLT_TOOL_BACKEND", "local")
            .env("VOLT_LANG", "en")
            .env("NO_COLOR", "1")
            .current_dir(&dir)
            .args(["test", "p.volt", "--target-dir", "build"])
            .output()
            .expect("volt test");
        let text = format!(
            "{}\n--- stderr\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        // CI günlüğünde testin Verilator'da koştuğu görünsün (atlanan test
        // de "ok" yazar).
        eprintln!("--- volt test (e5010_advice_tests::{tag})\n{text}");
        assert!(out.status.success(), "{tag}: {text}");
        assert!(text.contains("1 passed; 0 failed"), "{tag}: {text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

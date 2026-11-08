//! Hiç örneklenmemiş generic modül (şablon) için SystemVerilog üretilmez,
//! W1006 verilir (#143, ADR-0111). Örneklenen generic modülün monomorfu
//! ve generic olmayan modül değişmez.
//!
//! Düzeltmeden önce şablon `volt build`'de SV modülü olarak yazılıyordu;
//! gövdesindeki generic parametre, aynı adlı bir kök const varsa onun
//! değerine çözülüyordu (`assign y = 32'd7;`, Verilator WIDTHTRUNC).
//! Şablonun parametre değeri yoktur (ADR-0041: şablon dokunulmadan kalır;
//! ADR-0056: örneklenmemiş şablon açılmaz).
//!
//! Uyarı yalnız ana dosyanın şablonu için: `use` ile yüklenen kütüphane
//! dosyasının modülleri zaten çıktı kümesine girmez (ADR-0042 eki,
//! `reach.rs`), orada davranış değişmez.

mod tools;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// #171: parametreli SV önerisi; uyarının metni onu gösterir.
const PLANNED: &str = "parameterized output is planned: #171";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-ungeneric-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt <cmd> --format json <file>` `dir`'de; (çıkış kodu, zarf).
fn run_json(dir: &Path, cmd: &str, file: &str) -> (Option<i32>, Value) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_volt"));
    c.current_dir(dir)
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .args([cmd, "--format", "json"]);
    if cmd == "build" {
        c.args(["--target-dir", "build"]);
    }
    let out = c.arg(file).output().expect("volt çalışmalı");
    let env: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "JSON değil ({e}): {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out.status.code(), env)
}

/// Zarftaki `code` tanılarının (satır, ileti) listesi.
fn diags(env: &Value, code: &str) -> Vec<(u64, String)> {
    env["diagnostics"]
        .as_array()
        .expect("tanılar")
        .iter()
        .filter(|d| d["code"] == code)
        .map(|d| {
            (
                d["spans"][0]["start"]["line"].as_u64().unwrap_or(0),
                d["message"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// `build/rtl/` altındaki `.sv` dosyalarının adları (sıralı).
fn rtl_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join("build/rtl"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".sv"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// #143'ün örneği: şablonun parametresi aynı adlı kök const'u gölgeler.
const ISSUE_143: &str = "const N : u32 = 7

module Fir<const N: u32> {
    out y : u8
    y = N
}
";

#[test]
fn issue_143_uninstantiated_generic_module_is_not_generated_and_warns() {
    let dir = temp_dir("issue");
    std::fs::write(dir.join("fir.volt"), ISSUE_143).expect("yaz");
    let (code, env) = run_json(&dir, "build", "fir.volt");
    assert_eq!(code, Some(0), "{env:#}");
    // Şablonun dosyası yok (düzeltmeden önce `Fir.sv`, `assign y = 32'd7;`).
    assert!(
        !rtl_files(&dir).iter().any(|n| n.starts_with("Fir")),
        "{:?}",
        rtl_files(&dir)
    );
    let w = diags(&env, "W1006");
    assert_eq!(w.len(), 1, "{env:#}");
    assert_eq!(w[0].0, 3, "{env:#}");
    assert!(
        w[0].1.contains(
            "module 'Fir' is generic and is never instantiated; \
             no SystemVerilog is generated for it"
        ),
        "{}",
        w[0].1
    );
    assert!(w[0].1.contains(PLANNED), "{}", w[0].1);
    // Gölgeleme uyarısı değişmedi (name-resolution.md §6, #122).
    assert_eq!(diags(&env, "W1002").len(), 1, "{env:#}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Yalnız şablon içeren dosya kütüphane değildir: `volt build` "no module
/// … library file" notunu basmaz (W1006 nedeni söyler); çıkış 0.
#[test]
fn issue_143_template_only_file_is_not_called_a_library() {
    let dir = temp_dir("human");
    std::fs::write(dir.join("fir.volt"), ISSUE_143).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .current_dir(&dir)
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .args(["build", "--target-dir", "build", "fir.volt"])
        .output()
        .expect("volt");
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{text}");
    assert!(text.contains("warning[W1006]"), "{text}");
    assert!(text.contains(PLANNED), "{text}");
    assert!(!text.contains("library file"), "{text}");
    assert!(text.contains("0 SV file(s)"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `volt check` aynı uyarıyı verir (ADR-0070: check ve build aynı tanılar).
#[test]
fn issue_143_check_reports_the_same_warning() {
    let dir = temp_dir("check");
    std::fs::write(dir.join("fir.volt"), ISSUE_143).expect("yaz");
    let (code, env) = run_json(&dir, "check", "fir.volt");
    assert_eq!(code, Some(0), "{env:#}");
    let w = diags(&env, "W1006");
    assert_eq!(w.len(), 1, "{env:#}");
    assert!(w[0].1.contains(PLANNED), "{}", w[0].1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Yanında generic olmayan bir modül varsa o yazılır, şablon yazılmaz.
#[test]
fn ui_fail_244_uninstantiated_generic_module_w1006() {
    let src = std::fs::read_to_string(
        root().join("tests/ui/fail/244_uninstantiated_generic_module.volt"),
    )
    .expect("fixture");
    let dir = temp_dir("ui244");
    std::fs::write(dir.join("case.volt"), &src).expect("yaz");
    let marker = src
        .lines()
        .position(|l| l.trim_start().starts_with("//~^ ERROR"))
        .expect("//~^ ERROR satırı") as u64; // üst satır (1 tabanlı)
    let (code, env) = run_json(&dir, "build", "case.volt");
    assert_eq!(code, Some(0), "{env:#}");
    let w = diags(&env, "W1006");
    assert_eq!(
        w.iter().map(|d| d.0).collect::<Vec<_>>(),
        [marker],
        "{env:#}"
    );
    assert!(w[0].1.contains(PLANNED), "{}", w[0].1);
    assert_eq!(rtl_files(&dir), ["Top.sv"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Örneklenen generic modül: monomorfu yazılır, şablon yazılmaz, uyarı
/// yok (ui/pass 139).
#[test]
fn issue_143_instantiated_generic_module_is_generated_as_before() {
    let src =
        std::fs::read_to_string(root().join("tests/ui/pass/139_generic_module_instantiated.volt"))
            .expect("fixture");
    let dir = temp_dir("inst");
    std::fs::write(dir.join("case.volt"), &src).expect("yaz");
    let (code, env) = run_json(&dir, "build", "case.volt");
    assert_eq!(code, Some(0), "{env:#}");
    assert!(diags(&env, "W1006").is_empty(), "{env:#}");
    assert_eq!(rtl_files(&dir), ["Fir_4.sv", "Top.sv"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Generic olmayan, hiç örneklenmemiş modül bugünkü gibi yazılır (her
/// modül kendi dosyasında, ADR-0024); uyarı yok.
#[test]
fn issue_143_non_generic_uninstantiated_modules_are_generated_as_before() {
    let dir = temp_dir("plain");
    std::fs::write(
        dir.join("two.volt"),
        "module A {
    in  x : u8
    out y : u8
    y = x
}

module B {
    in  x : u8
    out y : u8
    y = x
}
",
    )
    .expect("yaz");
    let (code, env) = run_json(&dir, "build", "two.volt");
    assert_eq!(code, Some(0), "{env:#}");
    assert!(diags(&env, "W1006").is_empty(), "{env:#}");
    assert_eq!(rtl_files(&dir), ["A.sv", "B.sv"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `use` ile yüklenen kütüphane dosyasındaki şablon uyarı vermez: ana
/// dosya ondan yalnız bir fn alıyor; kütüphanenin modülleri çıktıya
/// girmez (ADR-0042 eki), 0.1.0'da da girmiyordu.
#[test]
fn issue_143_template_in_a_library_file_does_not_warn() {
    let dir = temp_dir("lib");
    std::fs::write(
        dir.join("lib.volt"),
        "pub fn inc(a: u8) -> u9 {
    a + 1
}

pub module Fifo<const D: u32> {
    out y : u8
    y = D
}
",
    )
    .expect("yaz");
    std::fs::write(
        dir.join("main.volt"),
        "use lib::inc

module Top {
    in  a : u8
    out b : u9
    b = inc(a)
}
",
    )
    .expect("yaz");
    let (code, env) = run_json(&dir, "build", "main.volt");
    assert_eq!(code, Some(0), "{env:#}");
    assert!(diags(&env, "W1006").is_empty(), "{env:#}");
    assert_eq!(rtl_files(&dir), ["Top.sv"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Örneklenen generic modülün davranışı değişmedi: #143'ün örneği
/// örneklenince gövdedeki `N` parametredir (name-resolution.md §6), kök
/// const değil: `Fir<3>`'te y = 3. Verilator'da (WIDTH uyarısı da
/// `volt test`'i durdurur).
#[test]
fn issue_143_instantiated_template_reads_its_parameter_in_simulation() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    std::fs::write(
        dir.join("case.volt"),
        format!(
            "{ISSUE_143}
module Top {{
    out z : u8
    let f = Fir<3> {{ }}
    z = f.y
}}
test \"the parameter, not the root const\" {{
    let dut = Top {{ }};
    step(1);
    assert_eq(dut.z, 3);
}}
"
        ),
    )
    .expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["test", "case.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("--- volt test (uninstantiated_generic_tests::sim)\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(text.contains("1 passed; 0 failed"), "{text}");
    assert!(!text.contains("W1006"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

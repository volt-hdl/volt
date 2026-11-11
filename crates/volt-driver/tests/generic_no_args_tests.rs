//! #249: generic parametrelerin varsayılan değeri yoktur (gramerde
//! `GenericParam = "const" Ident ":" Type`); generic bir modülün
//! argümansız örneği 0 argüman verir: ADR-0041'in sayı kuralı, E2003.
//! Düzeltmeden önce kabul ediliyordu: `volt build` 0 ile çıkıyor, hiç
//! üretilmeyen bir modülü örnekleyen `Top.sv` yazıyordu (tek tanı yanıltıcı
//! bir W1006'ydı); başka bir açılım şablonun adını kurmuşsa (`M<1>` →
//! `M_1`) örnek o açılımın gövdesine sessizce bağlanıyordu.
//!
//! Davranış: tanının kodu, satırı ve W1006'nın yokluğu (`volt check` /
//! `volt build` JSON zarfı), `volt build`'in dosya yazmaması; çok dosyalı
//! birim; güvenli ikizin ürettiği modül adları; gerçek Verilator'da her
//! örneğin kendi gövdesini hesaplaması (beklenen değerler kaynaktan elle)
//! ve sessiz yanlış biçimin simülasyona hiç gelmemesi. Üretilen SV'nin
//! metni karşılaştırılmaz.
//!
//! Verilator yoksa simülasyon testleri atlanır; `VOLT_REQUIRE_TOOLS` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Testler paralel koşar: her test kendi `tag`'ini kullanır.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-noargs-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt <cmd> --format json <file>` `dir`'de; (çıkış kodu, zarf).
fn run_json(dir: &Path, cmd: &str, file: &Path) -> (Option<i32>, Value) {
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

fn diagnostics(env: &Value) -> &Vec<Value> {
    env["diagnostics"].as_array().expect("tanılar")
}

fn codes(env: &Value) -> Vec<String> {
    diagnostics(env)
        .iter()
        .map(|d| d["code"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// Birincil konumun satırı (1 tabanlı).
fn primary_line(d: &Value) -> Option<u64> {
    d["spans"]
        .as_array()?
        .iter()
        .find(|s| s["primary"] == true)?["start"]["line"]
        .as_u64()
}

/// 1 tabanlı satır numarası: `needle`'ı içeren ilk satır.
fn line_of(src: &str, needle: &str) -> u64 {
    let i = src
        .lines()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("satır yok: {needle}"));
    u64::try_from(i + 1).expect("küçük dosya")
}

/// ui/fail fikstürü: tek tanı E2003 (W1006 yok), birincil konum
/// `//~^ ERROR` işaretinin bir üstündeki satırda, ileti işaretteki metni
/// içerir. `volt build` 1 ile çıkar ve hiçbir dosya yazmaz.
fn assert_no_args_fixture(name: &str) {
    let file = root().join("tests/ui/fail").join(name);
    let src = std::fs::read_to_string(&file).expect("okunmalı");
    assert!(src.starts_with("//~ E2003"), "{name}");
    let part = src
        .lines()
        .find_map(|l| l.trim_start().strip_prefix("//~^ ERROR "))
        .expect("//~^ ERROR satırı")
        .trim()
        .to_string();

    let (code, env) = run_json(&root(), "check", &file);
    assert_eq!(code, Some(1), "{env}");
    assert_eq!(codes(&env), ["E2003"], "yalnız E2003, W1006 yok: {env}");
    let d = &diagnostics(&env)[0];
    assert!(
        d["message"].as_str().is_some_and(|m| m.contains(&part)),
        "{name}: '{part}' bekleniyor: {d}"
    );
    assert_eq!(
        primary_line(d),
        Some(line_of(&src, "//~^ ERROR") - 1),
        "{d}"
    );

    let dir = temp_dir(name.trim_end_matches(".volt"));
    let (code, env) = run_json(&dir, "build", &file);
    assert_eq!(code, Some(1), "{env}");
    assert!(
        !dir.join("build").exists(),
        "hatalı tasarım dosya yazmamalı: {env}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ui_fail_267_generic_instance_without_arguments() {
    assert_no_args_fixture("267_generic_instance_without_arguments.volt");
}

#[test]
fn ui_fail_268_generic_instance_without_arguments_bound_elsewhere() {
    assert_no_args_fixture("268_generic_instance_without_arguments_bound_elsewhere.volt");
}

#[test]
fn ui_fail_269_generic_instance_without_arguments_reversed() {
    assert_no_args_fixture("269_generic_instance_without_arguments_reversed.volt");
}

/// Çok dosyalı birim: şablonlar iki kütüphane dosyasında, argümansız örnek
/// ana dosyada. Tek tanı E2003, ana dosyada.
#[test]
fn multifile_generic_instance_without_arguments_is_e2003() {
    let file = root().join("tests/ui/multifile/genericnoargs/main.volt");
    let src = std::fs::read_to_string(&file).expect("okunmalı");
    let (code, env) = run_json(&root(), "check", &file);
    assert_eq!(code, Some(1), "{env}");
    assert_eq!(codes(&env), ["E2003"], "yalnız E2003, W1006 yok: {env}");
    let d = &diagnostics(&env)[0];
    assert_eq!(primary_line(d), Some(line_of(&src, "let r = M_1 {")), "{d}");
    let file_of_primary = d["spans"]
        .as_array()
        .expect("spans")
        .iter()
        .find(|s| s["primary"] == true)
        .and_then(|s| s["file"].as_str())
        .unwrap_or_default()
        .replace('\\', "/");
    assert!(file_of_primary.ends_with("main.volt"), "{d}");
}

/// Güvenli ikiz: `M<1>` `M_1` adını kurar, generic olmayan `M_2`
/// argümansız örneklenir; üç modül de yazılır.
#[test]
fn ui_pass_151_builds_each_module() {
    let file = root().join("tests/ui/pass/151_generic_and_plain_instances.volt");
    let dir = temp_dir("pass151");
    let (code, env) = run_json(&dir, "build", &file);
    assert_eq!(code, Some(0), "{env}");
    assert!(codes(&env).is_empty(), "{env}");
    let mut names: Vec<String> = std::fs::read_dir(dir.join("build/rtl"))
        .expect("rtl")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["M_1.sv", "M_2.sv", "Top.sv"]);
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ Davranış: Verilator ═══════════════════════════════════════════════

/// `volt test` (yerel araçlar); (başarılı mı, çıktı).
fn volt_test(tag: &str, src: &str) -> Option<(bool, String)> {
    tools::require(tools::Tool::Verilator)?;
    let dir = temp_dir(tag);
    std::fs::write(dir.join("case.volt"), src).expect("yaz");
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
    // CI günlüğünde testin Verilator'da koştuğu görünsün.
    eprintln!("--- volt test (generic_no_args_tests::{tag})\n{text}");
    let _ = std::fs::remove_dir_all(&dir);
    Some((out.status.success(), text))
}

/// `M<1>`: y = x + 1; `M_2`: y = x ^ 2. x = 5 → a = 6, b = 5 ^ 2 = 7;
/// x = 200 (0b1100_1000) → a = 201, b = 0b1100_1010 = 202.
#[test]
fn ui_pass_151_each_instance_computes_its_own_body() {
    let design =
        std::fs::read_to_string(root().join("tests/ui/pass/151_generic_and_plain_instances.volt"))
            .expect("oku");
    let test = r#"
test "each instance computes its own body" {
    let dut = Top { };
    dut.x = 5;
    step(1);
    assert_eq(dut.a, 6);
    assert_eq(dut.b, 7);
    dut.x = 200;
    step(1);
    assert_eq(dut.a, 201);
    assert_eq(dut.b, 202);
}
"#;
    let Some((ok, text)) = volt_test("pass151-sim", &format!("{design}{test}")) else {
        return;
    };
    assert!(ok, "{text}");
    assert!(text.contains("1 passed; 0 failed"), "{text}");
}

/// #249'un sessiz yanlış biçimi (fail/268): düzeltmeden önce `r = M_1 { }`
/// `M<1>`'in gövdesine bağlanıyor, `c` x + 1 hesaplıyordu (x = 5 → 6) ve
/// aşağıdaki test geçiyordu. Argümansız generic örnek için şartnamede bir
/// değer yok; tasarım E2003 ile reddedilir, simülasyona hiç gelmez.
#[test]
fn issue_249_bound_form_is_rejected_instead_of_simulating_another_body() {
    let design = std::fs::read_to_string(
        root().join("tests/ui/fail/268_generic_instance_without_arguments_bound_elsewhere.volt"),
    )
    .expect("oku");
    let test = r#"
test "the instance without arguments" {
    let dut = Top { };
    dut.x = 5;
    step(1);
    assert_eq(dut.a, 6);
    assert_eq(dut.b, 0);
    assert_eq(dut.c, 6);
}
"#;
    let Some((ok, text)) = volt_test("issue249", &format!("{design}{test}")) else {
        return;
    };
    assert!(!ok, "{text}");
    assert!(text.contains("error[E2003]"), "{text}");
    assert!(!text.contains("passed;"), "simülasyon koşmamalı: {text}");
}

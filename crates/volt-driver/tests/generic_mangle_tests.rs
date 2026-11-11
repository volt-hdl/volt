//! #205: bir generic örnek `<modül>_<argümanlar>` modül adını alır
//! (sv-mapping.md §9, ADR-0041). İki FARKLI generic modül aynı adı
//! kurarsa (`M<1, 8>` ve `M_1<8>` → `M_1_8`) Volt adı değiştirmez
//! (ADR-0090): E1003. Düzeltmeden önce ikinci örnek birincinin gövdesine
//! bağlanıyordu ve tek tanı yanıltıcı bir W1006'ydı.
//!
//! Davranış: tanının kodu, satırı, dört konumu ve W1006'nın yokluğu
//! (`volt check`/`volt build` JSON zarfı); çok dosyalı birim; güvenli
//! ikizin ürettiği modül adları; ve gerçek Verilator'da her örneğin kendi
//! gövdesini hesaplaması (beklenen değerler kaynaktan elle). Üretilen
//! SV'nin metni karşılaştırılmaz.
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

/// Testler paralel koşar: her test kendi `tag`'ini kullanır (iki test aynı
/// dizini paylaşınca biri ötekinin dizinini siliyordu, CI 43087288935).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-mangle-{tag}-{}", std::process::id()));
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

/// Tanının bütün konumları: (satır, birincil mi).
fn span_lines(d: &Value) -> Vec<(u64, bool)> {
    d["spans"]
        .as_array()
        .expect("spans")
        .iter()
        .map(|s| {
            (
                s["start"]["line"].as_u64().expect("satır"),
                s["primary"] == true,
            )
        })
        .collect()
}

/// 1 tabanlı satır numarası: `needle`'ı içeren ilk satır.
fn line_of(src: &str, needle: &str) -> u64 {
    let i = src
        .lines()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("satır yok: {needle}"));
    u64::try_from(i + 1).expect("küçük dosya")
}

/// ui/fail fikstürü: tek tanı E1003, `//~^ ERROR` satırında, iletisi
/// parçayı içerir; dört konum (iki örnek, iki şablon); W1006 yok.
/// `volt build` 1 ile çıkar ve SV yazmaz.
fn assert_collision_fixture(name: &str, primary: &str, other: &str) {
    let file = root().join("tests/ui/fail").join(name);
    let src = std::fs::read_to_string(&file).expect("okunmalı");
    let part = src
        .lines()
        .find_map(|l| l.trim_start().strip_prefix("//~^ ERROR "))
        .expect("//~^ ERROR satırı")
        .trim()
        .to_string();

    let (code, env) = run_json(&root(), "check", &file);
    assert_eq!(code, Some(1), "{env}");
    assert_eq!(codes(&env), ["E1003"], "yalnız E1003, W1006 yok: {env}");
    let d = &diagnostics(&env)[0];
    assert!(
        d["message"].as_str().is_some_and(|m| m.contains(&part)),
        "{name}: '{part}' bekleniyor: {d}"
    );
    assert_eq!(primary_line(d), Some(line_of(&src, primary)), "{d}");
    let mut lines = span_lines(d);
    lines.sort_unstable();
    let mut want = vec![
        (line_of(&src, primary), true),
        (line_of(&src, other), false),
        (line_of(&src, "module M<"), false),
        (line_of(&src, "module M_1<"), false),
    ];
    want.sort_unstable();
    assert_eq!(lines, want, "{d}");
    let help = d["help"].as_str().unwrap_or_default();
    assert!(help.contains("'M'") && help.contains("'M_1'"), "{d}");

    let dir = temp_dir(name.trim_end_matches(".volt"));
    let (code, env) = run_json(&dir, "build", &file);
    assert_eq!(code, Some(1), "{env}");
    assert!(
        !dir.join("build/rtl").exists(),
        "hatalı tasarım SV yazmamalı: {env}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ui_fail_265_generic_mangle_collision() {
    assert_collision_fixture(
        "265_generic_mangle_collision.volt",
        "let q = M_1<8>",
        "let p = M<1, 8>",
    );
}

#[test]
fn ui_fail_266_generic_mangle_collision_reversed() {
    assert_collision_fixture(
        "266_generic_mangle_collision_reversed.volt",
        "let p = M<1, 8>",
        "let q = M_1<8>",
    );
}

/// Çok dosyalı birim: şablonlar iki kütüphane dosyasında, örnekler ana
/// dosyada. Tek tanı E1003, iki şablon adı kendi dosyalarında etiketli.
#[test]
fn multifile_generic_mangle_collision_is_e1003() {
    let file = root().join("tests/ui/multifile/genericmangle/main.volt");
    let (code, env) = run_json(&root(), "check", &file);
    assert_eq!(code, Some(1), "{env}");
    assert_eq!(codes(&env), ["E1003"], "yalnız E1003, W1006 yok: {env}");
    let files: Vec<String> = diagnostics(&env)[0]["spans"]
        .as_array()
        .expect("spans")
        .iter()
        .map(|s| {
            let f = s["file"].as_str().unwrap_or_default().replace('\\', "/");
            f.rsplit('/').next().unwrap_or_default().to_string()
        })
        .collect();
    for want in ["main.volt", "liba.volt", "libb.volt"] {
        assert!(files.iter().any(|f| f == want), "{want}: {files:?}");
    }
}

/// Güvenli ikiz: iki şablon iki ayrı modül adına iner (`M_1_8`, `M_1_7`);
/// ikisi de yazılır.
#[test]
fn ui_pass_150_builds_two_module_names() {
    let file = root().join("tests/ui/pass/150_generic_distinct_mangled_names.volt");
    let dir = temp_dir("pass150");
    let (code, env) = run_json(&dir, "build", &file);
    assert_eq!(code, Some(0), "{env}");
    assert!(codes(&env).is_empty(), "{env}");
    let mut names: Vec<String> = std::fs::read_dir(dir.join("build/rtl"))
        .expect("rtl")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["M_1_7.sv", "M_1_8.sv", "Top.sv"]);
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ Davranış: Verilator'da her örnek kendi gövdesini hesaplar ═════════

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
    eprintln!("--- volt test (generic_mangle_tests::{tag})\n{text}");
    let _ = std::fs::remove_dir_all(&dir);
    Some((out.status.success(), text))
}

/// `M<1, 8>`: y = x + 1; `M_1<7>`: y = x ^ 7. x = 5 → a = 6, b = 5 ^ 7
/// = 2; x = 200 (0b1100_1000) → a = 201, b = 0b1100_1111 = 207.
const PASS_150_TEST: &str = r#"
test "each instance computes its own body" {
    let dut = Top { };
    dut.x = 5;
    step(1);
    assert_eq(dut.a, 6);
    assert_eq(dut.b, 2);
    dut.x = 200;
    step(1);
    assert_eq(dut.a, 201);
    assert_eq(dut.b, 207);
}
"#;

#[test]
fn ui_pass_150_each_instance_computes_its_own_body() {
    let design = std::fs::read_to_string(
        root().join("tests/ui/pass/150_generic_distinct_mangled_names.volt"),
    )
    .expect("oku");
    let Some((ok, text)) = volt_test("pass150-sim", &format!("{design}{PASS_150_TEST}")) else {
        return;
    };
    assert!(ok, "{text}");
    assert!(text.contains("1 passed; 0 failed"), "{text}");
}

/// #205'in örneği: `q = M_1<8>` x ^ 8 hesaplamalı (x = 5 → 13), `p =
/// M<1, 8>` x + 1 (x = 5 → 6). Düzeltmeden önce tasarım kabul ediliyor ve
/// `q` da `M_1_8` = x + 1 hesaplıyordu (b = 6). Kabul edilmesi ancak her
/// örnek kendi gövdesini hesaplarsa doğrudur; ADR-0090'ın kuralıyla
/// E1003'le reddedilir, simülasyona hiç gelmez.
#[test]
fn issue_205_example_is_rejected_instead_of_simulating_the_wrong_body() {
    let design =
        std::fs::read_to_string(root().join("tests/ui/fail/265_generic_mangle_collision.volt"))
            .expect("oku");
    let test = r#"
test "each instance computes its own body" {
    let dut = Top { };
    dut.x = 5;
    step(1);
    assert_eq(dut.a, 6);
    assert_eq(dut.b, 13);
}
"#;
    let Some((ok, text)) = volt_test("issue205", &format!("{design}{test}")) else {
        return;
    };
    assert!(!ok, "{text}");
    assert!(text.contains("error[E1003]"), "{text}");
    assert!(!text.contains("passed;"), "simülasyon koşmamalı: {text}");
}

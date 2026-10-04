//! `volt check` test dosyasının test bloklarını da denetler: kardeş
//! `X.volt` (DUT) yüklenir, `volt test` ile aynı tam denetim (E8501-E8512)
//! koşar. Önceden `dut.countx` geçiyor ve "volt build X_test.volt"
//! öneriliyordu. Verilator gerekmez.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-tcheck-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    // `.git` tavanı: Volt.toml araması bu dizinin üstüne çıkmaz (ADR-0061).
    std::fs::create_dir_all(dir.join(".git")).expect(".git");
    dir
}

const COUNTER: &str = "pub module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    reg count_r : u8 = 0
    on clk {
        if enable {
            count_r <= count_r + 1
        }
    }
    count = count_r
}
";

const GOOD_TEST: &str = "test \"counts\" {
    let dut = Counter { };
    dut.enable = true;
    step(3);
    assert_eq(dut.count, 3);
}
";

/// Kardeş `counter.volt` + `counter_test.volt` (`test` içeriği); `manifest`
/// ise Volt.toml da yazılır.
fn setup(tag: &str, test: &str, manifest: bool) -> PathBuf {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("counter.volt"), COUNTER).expect("yaz");
    std::fs::write(dir.join("counter_test.volt"), test).expect("yaz");
    if manifest {
        std::fs::write(
            dir.join("Volt.toml"),
            "[package]\nname = \"p\"\nsrc = \".\"\ntop = \"Counter\"\n",
        )
        .expect("yaz");
    }
    dir
}

fn volt(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(args)
        .current_dir(dir)
        .env("VOLT_LANG", "en")
        .env_remove("VOLT_MANIFEST_DIR")
        .output()
        .expect("volt çalışmalı")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn check_of_a_test_file_reports_an_unknown_port() {
    let dir = setup(
        "unknown",
        &GOOD_TEST.replace("dut.count,", "dut.countx,"),
        false,
    );
    let out = volt(&dir, &["check", "counter_test.volt"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("error[E8502]"), "{err}");
    assert!(err.contains("'countx'"), "{err}");
    // Tek tanı: `volt test` ile aynı denetim bir kez sayılır.
    assert_eq!(err.matches("error[E8502]").count(), 1, "{err}");
    assert!(err.contains("Result 1 error(s)"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_of_a_clean_test_file_suggests_volt_test() {
    let dir = setup("next", GOOD_TEST, false);
    let out = volt(&dir, &["check", "counter_test.volt"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(
        err.contains("Next: volt test counter_test.volt   (run the tests)"),
        "{err}"
    );
    assert!(!err.contains("volt build counter_test.volt"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_of_a_design_file_still_suggests_volt_build() {
    let dir = setup("design", GOOD_TEST, false);
    let out = volt(&dir, &["check", "counter.volt"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(err.contains("Next: volt build counter.volt"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_check_covers_test_files() {
    let dir = setup(
        "project",
        &GOOD_TEST.replace("dut.count,", "dut.countx,"),
        true,
    );
    let out = volt(&dir, &["check"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("Checking counter.volt"), "{err}");
    assert!(err.contains("Checking counter_test.volt"), "{err}");
    assert_eq!(err.matches("error[E8502]").count(), 1, "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clean_project_check_lists_the_test_file_once() {
    let dir = setup("project-ok", GOOD_TEST, true);
    let out = volt(&dir, &["check"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert_eq!(
        err.matches("Checking counter_test.volt").count(),
        1,
        "{err}"
    );
    assert!(err.contains("Result 0 error(s), 0 warning(s)"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Okur testi: proje kipinde check ve test aynı dosyaları görür ────────
// `volt check` test dosyalarını `volt test`'in keşif kuralıyla bulur
// (alt dizinler dahil, build/ atlanır), her birini "Checking" satırıyla
// yazar ve iki komut aynı göreli yolu basar (`.\`/`./` öneki yok).

/// `rel` (proje köküne göre, `/` ayırıcılı) iletilerdeki yerel biçimiyle.
fn shown(rel: &str) -> String {
    rel.split('/').collect::<PathBuf>().display().to_string()
}

fn write_in(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().expect("üst dizin")).expect("dizin");
    std::fs::write(path, text).expect("yaz");
}

#[test]
fn project_check_fails_on_a_syntax_error_in_a_test_file() {
    let dir = setup("syntax", &format!("{GOOD_TEST}bu bir hata\n"), true);
    let out = volt(&dir, &["check"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("Checking counter_test.volt"), "{err}");
    assert!(err.contains("error[E0001]"), "{err}");
    assert!(err.contains("counter_test.volt:7:1"), "{err}");
    assert!(err.contains("Result 1 error(s)"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_check_covers_tests_in_subdirectories_and_skips_build() {
    let dir = setup("subdirs", GOOD_TEST, true);
    write_in(&dir, "sim/deep/deep_test.volt", "bu bir hata\n");
    write_in(&dir, "build/stale_test.volt", "build altindaki hata\n");
    write_in(&dir, "build/stale.volt", "build altindaki hata\n");
    let out = volt(&dir, &["check"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    let deep = shown("sim/deep/deep_test.volt");
    assert!(err.contains(&format!("Checking {deep}\n")), "{err}");
    assert!(err.contains(&format!("{deep}:1:1")), "{err}");
    assert_eq!(err.matches("error[E0001]").count(), 1, "{err}");
    assert!(!err.contains("stale"), "build/ taranmamalı: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_check_covers_a_source_the_top_does_not_use() {
    let dir = setup("orphan", GOOD_TEST, true);
    write_in(
        &dir,
        "orphan.volt",
        "module Orphan {\n    in  a : u8\n    out y : u8\n    y = a +\n}\n",
    );
    let out = volt(&dir, &["check"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("Checking orphan.volt\n"), "{err}");
    assert!(err.contains("orphan.volt:5:1"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// "Checking"/"Compiling" satırlarının yolları, geldiği sırayla.
fn listed(err: &str, verb: &str) -> Vec<String> {
    err.lines()
        .filter_map(|l| l.trim().strip_prefix(verb))
        .map(|p| p.trim().to_string())
        .collect()
}

#[test]
fn check_and_test_name_the_test_files_the_same_way() {
    // Sıradaki son test dosyası bozuk: `volt test` ikisini de derler ve
    // Verilator'a gelmeden durur.
    let dir = setup("paths", GOOD_TEST, true);
    write_in(&dir, "sim/deep_test.volt", "bu bir hata\n");
    let check = stderr(&volt(&dir, &["check"]));
    let test = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "test"])
        .current_dir(&dir)
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_VERILATOR", dir.join("no-such-verilator"))
        .env_remove("VOLT_MANIFEST_DIR")
        .output()
        .expect("volt çalışmalı");
    let test = stderr(&test);
    let expected = [shown("counter_test.volt"), shown("sim/deep_test.volt")];
    let checked: Vec<String> = listed(&check, "Checking ")
        .into_iter()
        .filter(|p| p.ends_with("_test.volt"))
        .collect();
    assert_eq!(checked, expected, "{check}");
    assert_eq!(listed(&test, "Compiling "), expected, "{test}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Kardeş `X.volt`'u olmayan `X_test.volt` (ör. `sim/deep_test.volt`, DUT
/// başka dizinde): `volt test` E8501 verir; `volt check` de aynı tam
/// denetimi yapmalı, "0 error(s)" demez.
#[test]
fn check_of_a_test_file_without_a_sibling_reports_what_volt_test_reports() {
    let dir = setup("nosibling", GOOD_TEST, true);
    write_in(&dir, "sim/deep_test.volt", GOOD_TEST);
    let out = volt(&dir, &["check"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("error[E8501]"), "{err}");
    assert!(err.contains(&shown("sim/deep_test.volt")), "{err}");
    // Tek dosya argümanıyla da aynı.
    let single = volt(&dir, &["check", "sim/deep_test.volt"]);
    assert_eq!(single.status.code(), Some(1), "{}", stderr(&single));
    assert!(
        stderr(&single).contains("error[E8501]"),
        "{}",
        stderr(&single)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

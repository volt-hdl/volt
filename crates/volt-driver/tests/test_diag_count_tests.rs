//! `volt test` tanı sayımı: her tanı BİR kez basılır ve BİR kez sayılır.
//!
//! `compile` test bloklarını tek dosya olarak, `compile_unit` ise kardeş
//! dosyayla birlikte denetler; ikisi aynı tanıyı üretince tekilleştirme
//! gerekir. Derleme hataları Verilator aranmadan önce raporlanır, bu
//! yüzden testler Verilator istemez.

use std::path::PathBuf;
use std::process::Command;

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-diagcount-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

const ECHO: &str = "\
module Echo {
    in  clk : clock
    in  d   : u4
    out q   : u4

    reg hold : u4 = 0

    on clk {
        hold <= d
    }

    q = hold
}
";

/// Dosyaları yazıp `volt test <test_file>` koşturur; (çıkış kodu, stderr).
fn run_test_cmd(tag: &str, files: &[(&str, &str)], test_file: &str) -> (Option<i32>, String) {
    let dir = temp_dir(tag);
    for (name, text) in files {
        std::fs::write(dir.join(name), text).expect("yaz");
    }
    let output = volt()
        .current_dir(&dir)
        .env("VOLT_LANG", "en")
        .args(["test", test_file, "--target-dir", "build"])
        .output()
        .expect("volt test");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    (output.status.code(), stderr)
}

fn run_single_file(tag: &str, tests: &str) -> (Option<i32>, String) {
    let source = format!("{ECHO}\n{tests}");
    run_test_cmd(tag, &[("echo_test.volt", &source)], "echo_test.volt")
}

fn occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

#[test]
fn single_error_is_printed_and_counted_once() {
    let (code, stderr) = run_single_file(
        "one",
        "test \"t\" {\n    let dut = Echo { };\n    dut.q = 1;\n}\n",
    );

    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E8503]"), 1, "{stderr}");
    assert!(stderr.contains("due to 1 error(s)"), "{stderr}");
}

#[test]
fn two_different_errors_are_counted_as_two() {
    let (code, stderr) = run_single_file(
        "two",
        "test \"t\" {\n    let dut = Echo { };\n    dut.q = 1;\n    dut.nope = 1;\n}\n",
    );

    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E8503]"), 1, "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E8502]"), 1, "{stderr}");
    assert!(stderr.contains("due to 2 error(s)"), "{stderr}");
}

#[test]
fn same_error_in_two_test_blocks_is_counted_per_block() {
    // Karar: tanı kimliği (kod, konum, ileti) üçlüsüdür. İki blok iki
    // ayrı konumdur; kullanıcı ikisini de düzeltmek zorundadır.
    let (code, stderr) = run_single_file(
        "blocks",
        "test \"a\" {\n    let dut = Echo { };\n    dut.q = 1;\n}\n\n\
         test \"b\" {\n    let dut = Echo { };\n    dut.q = 1;\n}\n",
    );

    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E8503]"), 2, "{stderr}");
    assert!(stderr.contains("due to 2 error(s)"), "{stderr}");
}

#[test]
fn builtin_arity_error_is_counted_once() {
    let (code, stderr) = run_single_file(
        "mixed",
        "test \"t\" {\n    let dut = Echo { };\n    step();\n}\n",
    );

    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E8505]"), 1, "{stderr}");
    assert!(stderr.contains("due to 1 error(s)"), "{stderr}");
}

#[test]
fn sibling_only_error_is_reported_once() {
    // Test dosyasında modül yok: `compile` modül-varlık denetimini atlar,
    // E8502'ü yalnız kardeşli tam denetim üretir.
    let tests = "test \"t\" {\n    let dut = Echo { };\n    dut.nope = 1;\n}\n";
    let (code, stderr) = run_test_cmd(
        "sibling",
        &[("echo.volt", ECHO), ("echo_test.volt", tests)],
        "echo_test.volt",
    );

    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E8502]"), 1, "{stderr}");
    assert!(stderr.contains("due to 1 error(s)"), "{stderr}");
}

#[test]
fn sibling_error_seen_by_both_passes_is_reported_once() {
    // E8505 modül bilgisi gerektirmez: hem tek dosya hem kardeşli
    // denetim üretir — tekilleştirmenin asıl hedefi.
    let tests = "test \"t\" {\n    let dut = Echo { };\n    step();\n}\n";
    let (code, stderr) = run_test_cmd(
        "sibling-both",
        &[("echo.volt", ECHO), ("echo_test.volt", tests)],
        "echo_test.volt",
    );

    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E8505]"), 1, "{stderr}");
    assert!(stderr.contains("due to 1 error(s)"), "{stderr}");
}

#[test]
fn non_test_error_is_not_affected_by_deduplication() {
    // Test dışı bir derleme hatası (çözülemeyen ad) yine bir kez çıkar.
    let source = "\
module Bad {
    in  clk : clock
    out q   : u4

    q = missing
}

test \"t\" {
    let dut = Bad { };
    step(1);
}
";
    let (code, stderr) = run_test_cmd("nontest", &[("bad_test.volt", source)], "bad_test.volt");

    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E"), 1, "{stderr}");
    assert!(stderr.contains("due to 1 error(s)"), "{stderr}");
}

// ═══ Kardeş dosyadaki `const` (ADR-0060) ══════════════════════════

const TABLE_LIB: &str = "\
const DEPTH : u8 = 9
const LAST  : u8 = 7

module M {
    in  clk  : clock
    in  addr : u3
    out echo : u3

    echo = addr
}
";

#[test]
fn sibling_const_overflow_is_one_e8512_not_an_undefined_name() {
    let tests = "test \"t\" {\n    let dut = M { };\n    dut.addr = DEPTH;\n    step(1);\n}\n";
    let (code, stderr) = run_test_cmd(
        "sibling-const",
        &[("m.volt", TABLE_LIB), ("m_test.volt", tests)],
        "m_test.volt",
    );

    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E8506]"), 0, "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E8512]"), 1, "{stderr}");
    assert!(stderr.contains("DEPTH = 9"), "{stderr}");
    assert!(stderr.contains("due to 1 error(s)"), "{stderr}");
}

#[test]
fn sibling_const_that_fits_builds_the_test() {
    // Derleme geçer; başlatılamayan "Verilator" ile araç aşamasına
    // (çıkış 3) kadar gelinmesi derlemenin hatasız bittiğini gösterir.
    let dir = temp_dir("sibling-const-ok");
    std::fs::write(dir.join("m.volt"), TABLE_LIB).expect("yaz");
    let test_file = dir.join("m_test.volt");
    std::fs::write(
        &test_file,
        "test \"t\" {\n    let dut = M { };\n    dut.addr = LAST;\n    step(1);\n    assert_eq(dut.echo, LAST);\n}\n",
    )
    .expect("yaz");
    let output = volt()
        .current_dir(&dir)
        .env("VOLT_LANG", "en")
        .env("VOLT_VERILATOR", &test_file)
        .args(["test", "m_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(output.status.code(), Some(3), "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E"), 0, "{stderr}");
}

#[test]
fn sibling_style_typo_is_still_reported_once_by_the_full_check() {
    let tests = "test \"t\" {\n    let dut = M { };\n    dut.addr = typo;\n}\n";
    let (code, stderr) = run_test_cmd(
        "sibling-typo",
        &[("m.volt", TABLE_LIB), ("m_test.volt", tests)],
        "m_test.volt",
    );

    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(occurrences(&stderr, "error[E8506]"), 1, "{stderr}");
    assert!(stderr.contains("due to 1 error(s)"), "{stderr}");
}

//! Test bloğunda port genişliği denetimi (ADR-0059) — CLI düzeyi.
//!
//! Derleme zamanı (E8512) testleri her ortamda koşar. Çalışma zamanı
//! testleri gerçek bir Verilator ister (`VOLT_VERILATOR` ya da `PATH`);
//! yoksa atlanır — CI'ın `cargo test` işi Verilator kurmaz, bu testler
//! Docker tarifinde (examples/README.md) koşar.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// İletiler metin olarak denetlenir: dil ortamdan bağımsız sabitlenir.
fn volt() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_volt"));
    cmd.env("VOLT_LANG", "en");
    cmd
}

fn ui(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui")).join(rel)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-bounds-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

const TABLE: &str = "\
module Table {
    in  clk  : clock
    in  addr : u3
    in  sv   : i8
    in  we   : bool
    out echo  : u3
    out secho : i8
    out neg   : bool

    reg hits : u8 = 0

    on clk {
        if we {
            hits <= hits + 1
        }
    }

    echo = addr
    secho = sv
    neg = sv < 0
}
";

fn write_test(dir: &Path, tests: &str) -> PathBuf {
    let file = dir.join("table_test.volt");
    std::fs::write(&file, format!("{TABLE}\n{tests}")).expect("yaz");
    file
}

fn check(tag: &str, tests: &str) -> (Output, String) {
    let dir = temp_dir(tag);
    let file = write_test(&dir, tests);
    let output = volt().arg("check").arg(&file).output().expect("volt check");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    (output, stderr)
}

// ═══ Derleme zamanı: E8512 ════════════════════════════════════════

#[test]
fn check_constant_overflow_is_e8512() {
    let (output, stderr) = check(
        "const",
        "test \"t\" {\n    let dut = Table { };\n    dut.addr = 8;\n}\n",
    );
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("error[E8512]"), "stderr: {stderr}");
    assert!(stderr.contains("value 8"), "stderr: {stderr}");
    assert!(
        stderr.contains("port 'addr' is u3 (max 7)"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("use a value in range 0..7"),
        "stderr: {stderr}"
    );
}

#[test]
fn check_boundary_value_is_clean() {
    let (output, stderr) = check(
        "edge",
        "test \"t\" {\n    let dut = Table { };\n    dut.addr = 7;\n    dut.sv = 0 - 128;\n    dut.sv = 255;\n}\n",
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
}

#[test]
fn check_signed_overflow_explains_the_negative_form() {
    let (output, stderr) = check(
        "signed",
        "test \"t\" {\n    let dut = Table { };\n    dut.sv = 0 - 129;\n}\n",
    );
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("error[E8512]"), "stderr: {stderr}");
    assert!(stderr.contains("(-129)"), "stderr: {stderr}");
    assert!(stderr.contains("range -128..255"), "stderr: {stderr}");
}

#[test]
fn check_computed_value_is_not_a_compile_error() {
    let (output, stderr) = check(
        "computed",
        "test \"t\" {\n    let dut = Table { };\n    for i in 0..16 {\n        dut.addr = i;\n    }\n}\n",
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
}

#[test]
fn check_unreachable_assert_constant_is_e8512() {
    let (output, stderr) = check(
        "assert",
        "test \"t\" {\n    let dut = Table { };\n    assert_ne(dut.echo, 8);\n}\n",
    );
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("error[E8512]"), "stderr: {stderr}");
    assert!(stderr.contains("can never match"), "stderr: {stderr}");
}

#[test]
fn ui_fixture_reports_e8512() {
    let output = volt()
        .arg("check")
        .arg(ui("fail/60_test_port_overflow_const.volt"))
        .output()
        .expect("volt check");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("error[E8512]"), "stderr: {stderr}");
}

#[test]
fn explain_covers_e8512_in_both_languages() {
    for (lang, needle) in [("en", "does not fit"), ("tr", "sığmıyor")] {
        let output = volt()
            .args(["explain", "--lang", lang, "E8512"])
            .output()
            .expect("volt explain");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(output.status.code(), Some(0), "{lang}: {stdout}");
        assert!(stdout.contains(needle), "{lang}: {stdout}");
    }
}

// ═══ Çalışma zamanı: gerçek Verilator ═════════════════════════════

fn have_verilator() -> bool {
    if std::env::var_os("VOLT_VERILATOR").is_some_and(|p| Path::new(&p).is_file()) {
        return true;
    }
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| {
            ["verilator", "verilator.exe"]
                .iter()
                .any(|name| dir.join(name).is_file())
        })
    })
}

/// `volt test` koşturur; Verilator yoksa `None` (test atlanır).
fn run_tests(tag: &str, tests: &str) -> Option<(Option<i32>, String)> {
    if !have_verilator() {
        eprintln!("atlandı: Verilator yok ({tag})");
        return None;
    }
    let dir = temp_dir(tag);
    write_test(&dir, tests);
    let output = volt()
        .current_dir(&dir)
        .args(["test", "table_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    Some((
        output.status.code(),
        format!("{stdout}\n--- stderr\n{stderr}"),
    ))
}

#[test]
fn runtime_values_up_to_the_maximum_pass() {
    let Some((code, out)) = run_tests(
        "rt-pass",
        "test \"in range\" {\n    let dut = Table { };\n    for i in 0..8 {\n        dut.addr = i;\n        step(1);\n        assert_eq(dut.echo, i);\n    }\n}\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(0), "{out}");
    assert!(out.contains("test result: ok. 1 passed; 0 failed"), "{out}");
}

#[test]
fn runtime_overflow_fails_the_test_with_the_loop_iteration() {
    let Some((code, out)) = run_tests(
        "rt-fail",
        "test \"table lookup\" {\n    let dut = Table { };\n    for i in 0..16 {\n        dut.addr = i;\n        step(1);\n    }\n}\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(
        out.contains("port 'addr' (u3) cannot hold value 8 at table_test.volt:"),
        "{out}"
    );
    assert!(out.contains("range: 0..7"), "{out}");
    assert!(out.contains("loop:  i = 8"), "{out}");
}

#[test]
fn runtime_overflow_is_reported_instead_of_a_later_assert() {
    // Sığan yazma normal çalışır (echo = 5); sığmayan yazma testi port
    // hatasıyla bitirir. "Değer porta ulaşmaz" sırası üreteç testinde
    // (checked_port_write_guards_before_the_assignment) korunur: düşen
    // testten sonra port gözlenemez.
    let Some((code, out)) = run_tests(
        "rt-nowrite",
        "test \"keeps last\" {\n    let dut = Table { };\n    let n = 5;\n    dut.addr = n;\n    step(1);\n    assert_eq(dut.echo, 5);\n    dut.addr = n + 3;\n}\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(out.contains("cannot hold value 8"), "{out}");
    assert!(!out.contains("assert_eq failed"), "{out}");
}

#[test]
fn runtime_signed_port_holds_negative_numbers_exactly() {
    let Some((code, out)) = run_tests(
        "rt-signed",
        "test \"negatives\" {\n    let dut = Table { };\n    let one = 1;\n    dut.sv = 0 - one;\n    step(1);\n    assert_true(dut.neg);\n    assert_eq(dut.secho, 0xFF);\n    dut.sv = 0 - (one * 128);\n    step(1);\n    assert_eq(dut.secho, 0x80);\n    dut.sv = 254 + one;\n    step(1);\n    assert_eq(dut.secho, 255);\n}\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(0), "{out}");
}

#[test]
fn runtime_signed_port_rejects_values_below_the_minimum() {
    let Some((code, out)) = run_tests(
        "rt-signed-min",
        "test \"too negative\" {\n    let dut = Table { };\n    let n = 129;\n    dut.sv = 0 - n;\n}\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(out.contains("port 'sv' (i8) cannot hold value"), "{out}");
    assert!(out.contains("(-129)"), "{out}");
    assert!(out.contains("range: -128..255"), "{out}");
}

#[test]
fn runtime_unsigned_port_rejects_a_negative_number() {
    let Some((code, out)) = run_tests(
        "rt-neg-unsigned",
        "test \"negative\" {\n    let dut = Table { };\n    let one = 1;\n    dut.addr = 0 - one;\n}\n",
    ) else {
        return;
    };
    assert_eq!(code, Some(5), "{out}");
    assert!(out.contains("port 'addr' (u3) cannot hold value"), "{out}");
    assert!(out.contains("(-1)"), "{out}");
}

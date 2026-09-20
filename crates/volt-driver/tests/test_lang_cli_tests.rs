//! Test dili genişletmesi (ADR-0058) — CLI düzeyi: `read_hex` gerçek
//! dosya sistemiyle çözülür, E8507/E8508/E8510 `volt check`te görünür
//! ve `volt explain` yeni kodları iki dilde açıklar.

use std::path::PathBuf;
use std::process::{Command, Output};

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn ui(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui")).join(rel)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-testlang-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

const ROM_TEST: &str = "\
module Rom {
    in  clk   : clock
    in  addr  : u2
    in  we    : bool
    in  wdata : u32
    out data  : u32

    reg mem    : [u32; 4] = [0; 4]
    reg data_r : u32 = 0

    on clk {
        if we {
            mem[addr] <= wdata
        }
        data_r <= mem[addr]
    }

    data = data_r
}

test \"rom\" {
    let dut = Rom { };
    let image = read_hex(\"rom.hex\");
    load(dut.mem, image);
}
";

/// `rom_test.volt` + isteğe bağlı `rom.hex` yazıp `volt check` koşturur.
fn check_with_hex(tag: &str, hex: Option<&str>) -> Output {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("rom_test.volt"), ROM_TEST).expect("yaz");
    if let Some(text) = hex {
        std::fs::write(dir.join("rom.hex"), text).expect("yaz");
    }
    let output = volt()
        .arg("check")
        .arg(dir.join("rom_test.volt"))
        .output()
        .expect("volt çalışmalı");
    let _ = std::fs::remove_dir_all(&dir);
    output
}

#[test]
fn check_reads_the_hex_file_next_to_the_test() {
    let output = volt()
        .arg("check")
        .arg(ui("pass/80_test_read_hex.volt"))
        .output()
        .expect("volt çalışmalı");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
    assert!(
        stderr.contains("0 error(s), 0 warning(s)"),
        "stderr: {stderr}"
    );
}

#[test]
fn check_valid_hex_is_clean() {
    let output = check_with_hex("ok", Some("00000013 00000093\n@3 DEADBEEF\n"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
}

#[test]
fn check_missing_hex_file_is_e8507() {
    let output = check_with_hex("missing", None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("error[E8507]"), "stderr: {stderr}");
    assert!(stderr.contains("rom.hex"), "stderr: {stderr}");
}

#[test]
fn check_path_outside_project_is_e8507() {
    let output = volt()
        .arg("check")
        .arg(ui("fail/59_test_file_outside_project.volt"))
        .output()
        .expect("volt çalışmalı");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("error[E8507]"), "stderr: {stderr}");
    assert!(stderr.contains("leaves the project"), "stderr: {stderr}");
}

#[test]
fn check_malformed_hex_is_e8508_with_line_number() {
    let output = check_with_hex("bad", Some("00000013\n0000ZZ93\n"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("error[E8508]"), "stderr: {stderr}");
    assert!(stderr.contains("line 2"), "stderr: {stderr}");
}

#[test]
fn check_hex_longer_than_the_memory_is_e8510() {
    let output = check_with_hex("long", Some("1 2 3 4 5\n"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("error[E8510]"), "stderr: {stderr}");
    assert!(stderr.contains("5 element(s)"), "stderr: {stderr}");
}

#[test]
fn explain_covers_the_new_codes_in_both_languages() {
    for code in ["E8507", "E8508", "E8509", "E8510", "E8511"] {
        for lang in ["en", "tr"] {
            let output = volt()
                .args(["explain", "--lang", lang, code])
                .output()
                .expect("volt çalışmalı");
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert_eq!(output.status.code(), Some(0), "{code}/{lang}");
            assert!(stdout.contains(code), "{code}/{lang}: {stdout}");
        }
    }
    let tr = volt()
        .args(["explain", "--lang", "tr", "E8507"])
        .output()
        .expect("volt çalışmalı");
    assert!(String::from_utf8_lossy(&tr.stdout).contains("proje"));
}

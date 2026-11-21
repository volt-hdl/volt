//! `volt test`, benzetimin beklenmedik bitişini testlerle uzlaştırır
//! (#377, ADR-0130) — uçtan uca, gerçek Verilator'da.
//!
//! Önceden sürücü benzetimin çıkış kodunu hiç okumuyordu ve yalnız
//! `VOLT-TEST-END` satırlarını sayıyordu: bir `extern` gövdesindeki
//! `$fatal` / `$error` testbench'i kesince yarıda kalan test ve ondan
//! sonrakiler rapordan sessizce düşüyor, koşu "ok" diyip 0 ile çıkıyordu.
//! Verilator 5.054 koşuyu keserken 1 ile çıkar — normal düşen testlerin
//! koduyla aynı — bu yüzden ayrım eksik `VOLT-TEST-END`'den yapılır.
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Değer 100'ü aşınca `<stop>` çağıran extern denetleyici + onu örnekleyen
/// tasarım. Testler `tests` ile eklenir.
fn design(tests: &str) -> String {
    format!(
        "\
@source(\"rtl/chk.sv\")
extern module Chk {{
    in  clk : clock
    in  v   : u8
}}

module M {{
    in  clk : clock
    in  a   : u8
    out q   : u8

    let c = Chk {{ clk: clk, v: a }}
    reg r : u8 = 0
    on clk {{ r <= a }}
    q = r
}}
{tests}"
    )
}

fn checker(stop: &str) -> String {
    format!(
        "module Chk(input logic clk, input logic [7:0] v);\n  \
         always @(posedge clk) if (v > 8'd100) {stop};\nendmodule\n"
    )
}

/// `dut.a = <a>` sonra `q == <want>` bekleyen test.
fn test_block(name: &str, a: u32, want: u32) -> String {
    format!(
        "\ntest \"{name}\" {{\n    let dut = M {{ }};\n    dut.a = {a};\n    step(1);\n    assert_eq(dut.q, {want});\n}}\n"
    )
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-sim-stop-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("rtl")).expect("temp dizini");
    dir
}

/// Tasarımı ve denetleyiciyi yazar, `volt test` koşar (Verilator yoksa `None`).
fn run(tag: &str, stop: &str, tests: &str) -> Option<(Output, PathBuf)> {
    let verilator = tools::require(tools::Tool::Verilator)?;
    let dir = temp_dir(tag);
    std::fs::write(dir.join("rtl/chk.sv"), checker(stop)).expect("chk.sv");
    let file = dir.join("m_test.volt");
    std::fs::write(&file, design(tests)).expect("tasarım");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["test", "--no-waves", "--color", "never", "--target-dir"])
        .arg(dir.join("build"))
        .arg(&file)
        .current_dir(&dir)
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("VOLT_VERILATOR", &verilator)
        .output()
        .expect("volt çalışmalı");
    Some((out, dir))
}

fn text(out: &Output) -> String {
    format!(
        "{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn issue_377_fatal_in_an_extern_fails_the_test_and_the_rest_are_not_run() {
    // #377'nin örneği: 2. test denetleyiciyi tetikler ($fatal), 3. test
    // (q = 1, beklenen 99) düşmeliydi ama hiç koşmaz. Önceden: "1 passed;
    // 0 failed", çıkış 0.
    let tests = [
        test_block("first ok", 5, 5),
        test_block("trips checker", 200, 200),
        test_block("never runs", 1, 99),
    ]
    .concat();
    let Some((out, dir)) = run("fatal", "$fatal(1, \"value out of range\")", &tests) else {
        return;
    };
    let all = text(&out);
    assert_eq!(out.status.code(), Some(5), "{all}");
    assert!(all.contains("test first_ok ... ok"), "{all}");
    assert!(all.contains("test trips_checker ... FAILED"), "{all}");
    assert!(all.contains("test never_runs ... not run"), "{all}");
    assert!(
        all.contains("the simulation stopped during this test (exit code 1)"),
        "{all}"
    );
    // Simülatörün kendi iletisi rapora taşınır.
    assert!(
        all.contains("%Fatal") && all.contains("value out of range"),
        "{all}"
    );
    assert!(
        all.contains("test result: FAILED. 1 passed; 1 failed; 1 not run"),
        "{all}"
    );
    cleanup(&dir);
}

#[test]
fn an_error_in_the_last_test_fails_it_although_no_test_is_left() {
    // Son test $error ile kesilir: başlamamış test yok ve süreç, normal
    // düşen bir koşu gibi 1 ile çıkar. Kalan test yine de düşer.
    let tests = [
        test_block("one", 5, 5),
        test_block("two", 6, 6),
        test_block("trips checker", 200, 200),
    ]
    .concat();
    let Some((out, dir)) = run("error", "$error(\"value out of range\")", &tests) else {
        return;
    };
    let all = text(&out);
    assert_eq!(out.status.code(), Some(5), "{all}");
    assert!(all.contains("test trips_checker ... FAILED"), "{all}");
    assert!(
        all.contains("test result: FAILED. 2 passed; 1 failed\n"),
        "{all}"
    );
    assert!(!all.contains("not run"), "{all}");
    cleanup(&dir);
}

#[test]
fn passing_and_failing_tests_keep_their_result_when_the_simulation_ends_normally() {
    // Denetleyici hiç tetiklenmez: sonuçlar ve özet satırı eskisi gibi.
    let tests = [test_block("passes", 5, 5), test_block("fails", 7, 8)].concat();
    let Some((out, dir)) = run("normal", "$fatal(1, \"unreachable\")", &tests) else {
        return;
    };
    let all = text(&out);
    assert_eq!(out.status.code(), Some(5), "{all}");
    assert!(all.contains("test passes ... ok"), "{all}");
    assert!(all.contains("test fails ... FAILED"), "{all}");
    assert!(all.contains("assert_eq failed at"), "{all}");
    assert!(
        all.contains("test result: FAILED. 1 passed; 1 failed\n"),
        "{all}"
    );
    assert!(
        !all.contains("not run") && !all.contains("simulation stopped"),
        "{all}"
    );
    cleanup(&dir);
}

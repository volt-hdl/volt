//! `volt test` reset portu olmayan modüllerde çalışır — uçtan uca, gerçek
//! Verilator'da. Önceden testbench her modülde `dut->rst` sürüyordu:
//! saatsiz (tamamen kombinasyonel) modül, `reset = none` alanı ve
//! `active_low` alanı (`rst_n`) C++ derlemesinde düşüyordu (ADR-0098).
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

const DESIGNS: &str = "\
domain Low { clock = posedge, reset = sync active_low }
domain NoRst { clock = posedge, reset = none }

module Adder {
    in  a   : u8
    in  b   : u8
    out sum : u9

    let s = a + b
    sum = s
}

module CountLow {
    in  clk : clock @Low
    out q   : u8

    reg r : u8 = 5
    on clk { r <= r + 1 }
    q = r
}

module Shift {
    in  clk : clock @NoRst
    in  d   : u8
    out q   : u8

    reg r : u8 = 0
    on clk { r <= d }
    q = r
}

test \"combinational adder without a clock\" {
    let dut = Adder { };
    dut.a = 255;
    dut.b = 1;
    step(1);
    assert_eq(dut.sum, 256);
    dut.a = 3;
    dut.b = 4;
    step(1);
    assert_eq(dut.sum, 7);
}

test \"active low reset releases to the initial value\" {
    let dut = CountLow { };
    assert_eq(dut.q, 5);
    step(1);
    assert_eq(dut.q, 6);
}

test \"domain without a reset\" {
    let dut = Shift { };
    dut.d = 7;
    step(1);
    assert_eq(dut.q, 7);
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-clockless-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn modules_without_an_rst_port_run_under_volt_test() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    std::fs::write(dir.join("designs_test.volt"), DESIGNS).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .current_dir(&dir)
        .env("VOLT_LANG", "en")
        .args(["test", "designs_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let out = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.status.code(), Some(0), "{out}");
    assert!(out.contains("test result: ok. 3 passed; 0 failed"), "{out}");
}

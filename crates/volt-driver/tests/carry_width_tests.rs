//! Tipsiz `let` taşma bitini korur (type-inference.md §3.3, ADR-0025) —
//! uçtan uca, gerçek Verilator'da. `let s = a + b` (u8 + u8) doğal
//! olarak u9'dur; `sum : u9 = s` 255 + 1 için 256 vermelidir.
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

const ADDER: &str = "\
module Adder {
    in  clk   : clock
    in  a     : u8
    in  b     : u8
    in  x     : i8
    in  y     : i8
    out sum   : u9
    out wrap  : u8
    out diff  : i9
    out prod  : u16

    let s = a + b
    sum  = s
    wrap = s
    let d = x - y
    diff = d
    let p = a * b
    prod = p
}

test \"carry of 255 + 1\" {
    let dut = Adder { };
    dut.a = 255;
    dut.b = 1;
    step(1);
    assert_eq(dut.sum, 256);
    assert_eq(dut.wrap, 0);
}

test \"signed difference and full product\" {
    let dut = Adder { };
    dut.x = 0 - 128;
    dut.y = 1;
    dut.a = 255;
    dut.b = 255;
    step(1);
    assert_eq(dut.diff, 0x17F);
    assert_eq(dut.prod, 65025);
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-carry-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn untyped_let_keeps_the_carry_in_simulation() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    std::fs::write(dir.join("adder_test.volt"), ADDER).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .current_dir(&dir)
        .env("VOLT_LANG", "en")
        .args(["test", "adder_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let out = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.status.code(), Some(0), "{out}");
    assert!(out.contains("test result: ok. 2 passed; 0 failed"), "{out}");
}

//! Bileşik değerden indexed part-select taşma bitini doğru okur (#112),
//! uçtan uca, gerçek Verilator'da. `a = b = 255` için `a + b = 510`
//! (`1_1111_1110`); `[5 +: 4]` ve `[8 -: 4]` bit 8..5'i okur: 15. Toplam
//! 8 bitte basılsaydı (254) 7 olurdu; `(a + b)[5 +: 4]` biçimi ise SV'de
//! sözdizimi hatasıydı.
//!
//! Verilator yoksa test atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

const DESIGN: &str = "\
module Carry {
    in  a  : u8
    in  b  : u8
    in  i  : u4
    out up : bits<4>
    out dn : bits<4>
    out vr : bits<4>

    up = (a + b)[5 +: 4]
    dn = (a + b)[8 -: 4]
    vr = (a + b)[i +: 4]
}
";

const TESTS: &str = "\
test \"the carry is selected\" {
    let dut = Carry { };
    dut.a = 255;
    dut.b = 255;
    dut.i = 5;
    step(1);
    assert_eq(dut.up, 15);
    assert_eq(dut.dn, 15);
    assert_eq(dut.vr, 15);
}

test \"low bits with a variable start\" {
    let dut = Carry { };
    dut.a = 0x12;
    dut.b = 0x01;
    dut.i = 0;
    step(1);
    assert_eq(dut.vr, 3);
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-psel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn part_select_of_a_sum_reads_the_carry_in_simulation() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    std::fs::write(dir.join("carry.volt"), DESIGN).expect("yaz");
    std::fs::write(dir.join("carry_test.volt"), TESTS).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["test", "carry_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{text}");
    assert!(text.contains("2 passed; 0 failed"), "{text}");
}

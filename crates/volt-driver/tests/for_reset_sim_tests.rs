//! `for` döngüsünde yazılan reg reset sonrası başlangıç değerini taşır —
//! uçtan uca, gerçek Verilator'da. Önceden reset dalı boş üretiliyordu ve
//! `[7; 2]` reg'i reset sonrası 0 okunuyordu (sv-mapping.md §7).
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

const FOR_RESET: &str = "\
module ForReset {
    in  clk : clock
    in  en  : bool
    out a   : u8
    out b   : u8

    reg r : [u8; 2] = [7; 2]
    reg s : u8 = 5

    on clk {
        for i in 0..2 {
            if en {
                r[i] <= r[i] + 1
            }
        }
        for i in 0..1 {
            s <= s + 1
        }
    }
    a = r[0]
    b = r[1] + s
}

test \"reset value survives a for loop write\" {
    let dut = ForReset { };
    dut.en = false;
    step(1);
    assert_eq(dut.a, 7);
    assert_eq(dut.b, 13);
}

test \"loop writes count up from the reset value\" {
    let dut = ForReset { };
    dut.en = true;
    step(2);
    assert_eq(dut.a, 9);
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-forreset-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn register_written_in_a_for_loop_resets_to_its_initial_value() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    std::fs::write(dir.join("for_reset_test.volt"), FOR_RESET).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .current_dir(&dir)
        .env("VOLT_LANG", "en")
        .args(["test", "for_reset_test.volt", "--target-dir", "build"])
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

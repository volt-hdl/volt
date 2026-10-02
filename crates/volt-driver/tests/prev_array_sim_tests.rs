//! Dizi literali içindeki `prev()` `volt test`'te belgelenen anlamı taşır —
//! uçtan uca, gerçek Verilator'da (ADR-0098 eki). Önceden toplanmıyordu ve
//! `$past` yedeğine düşüyordu: test içi `reset()` sonrasındaki ilk çevrimde
//! skaler yazım ihlal verirken aynı özelliğin dizi literalli yazımı reset
//! öncesi değeri okuyup geçiyordu (ADR-0040: reset sonrası ilk çevrimde
//! `prev(x) == 0`).
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

/// Aynı özellik iki yazımla. `r2` bir önceki çevrimin `a`'sını tutar,
/// reset değeri 7; `b` özelliği test içi `reset()`'ten önce kapalı tutar.
const PREV_ARRAY: &str = "\
module PrevArr {
    in  clk : clock
    in  a   : u8
    in  b   : bool
    out y   : u8

    reg r2 : [u8; 2] = [7; 2]
    on clk {
        r2[0] <= a
        r2[1] <= a
    }
    y = r2[0]

    invariant: !b || r2 == [prev(a), prev(a)]
}

module PrevScalar {
    in  clk : clock
    in  a   : u8
    in  b   : bool
    out y   : u8

    reg r2 : [u8; 2] = [7; 2]
    on clk {
        r2[0] <= a
        r2[1] <= a
    }
    y = r2[0]

    invariant: !b || r2[0] == prev(a) && r2[1] == prev(a)
}

test \"array literal prev after reset\" {
    let dut = PrevArr { };
    dut.a = 7;
    step(3);
    dut.b = true;
    reset();
    step(2);
    assert_eq(dut.y, 7);
}

test \"scalar prev after reset\" {
    let dut = PrevScalar { };
    dut.a = 7;
    step(3);
    dut.b = true;
    reset();
    step(2);
    assert_eq(dut.y, 7);
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-prevarray-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn prev_in_an_array_literal_and_scalar_prev_agree_after_reset() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    std::fs::write(dir.join("prev_array_test.volt"), PREV_ARRAY).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .current_dir(&dir)
        .env("VOLT_LANG", "en")
        .args(["test", "prev_array_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let out = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    // Reset sonrası ilk çevrimde prev(a) == 0 ama r2 == 7: iki yazım da
    // aynı çevrimde ihlal verir.
    assert!(
        out.contains("test result: FAILED. 0 passed; 2 failed"),
        "{out}"
    );
    assert!(
        out.contains("invariant: !b || r2 == [prev(a), prev(a)]"),
        "{out}"
    );
    assert!(
        out.contains("invariant: !b || r2[0] == prev(a) && r2[1] == prev(a)"),
        "{out}"
    );
    assert_eq!(out.matches("at cycle 6").count(), 2, "{out}");
}

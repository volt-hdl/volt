//! Bütün-struct karşılaştırmasında literal alanı, alanın tipine sığmalıdır
//! (#375) — uçtan uca, gerçek Verilator'da.
//!
//! Önceden testbench literalin alanlarını alan genişliğine maskeleyip
//! paketliyordu (`17 & 15 = 1`): `assert_eq(dut.q, P { a: 17, .. })`,
//! `q.a = 1` tutan donanımda geçiyordu. Bütün-struct yazımı ise her alanı
//! port yazımı gibi denetliyordu (sabit E8512, hesaplanmış değer koşuda
//! `port_overflow`; ADR-0059). Karşılaştırma artık aynı iki denetimden
//! geçer: sabit E8512, hesaplanmış değer koşuda testi düşürür.
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::{Command, Output};

const DESIGN: &str = "\
struct P {
    a : u4
    s : i4
    b : bool
}

module M {
    in  clk : clock
    in  x   : P
    out q   : P

    reg r : P = P { a: 0, s: 0, b: false }
    on clk { r <= x }
    q = r
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-struct-range-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Tasarım + testler tek dosyada; `volt test` koşar (Verilator yoksa `None`).
fn run(tag: &str, tests: &str) -> Option<(Output, String)> {
    let verilator = tools::require(tools::Tool::Verilator)?;
    let dir = temp_dir(tag);
    let file = dir.join("m_test.volt");
    std::fs::write(&file, format!("{DESIGN}{tests}")).expect("tasarım");
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
    let text = format!(
        "{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    Some((out, text))
}

#[test]
fn issue_375_a_literal_field_that_does_not_fit_is_rejected() {
    // #375'in örneği: q.a = 1; 17, u4 alanına sığmaz. Önceden `== 3ULL`
    // (17 & 15 = 1) ile karşılaştırılıp geçiyordu.
    let tests = "
test \"masked literal\" {
    let dut = M { };
    dut.x = P { a: 1, s: 0, b: true };
    step(1);
    assert_eq(dut.q, P { a: 17, s: 0, b: true });
}
";
    let Some((out, all)) = run("const", tests) else {
        return;
    };
    assert_eq!(out.status.code(), Some(1), "{all}");
    assert!(all.contains("error[E8512]"), "{all}");
    assert!(all.contains("value 17"), "{all}");
    assert!(!all.contains("passed"), "{all}");
}

#[test]
fn a_computed_field_that_does_not_fit_fails_the_test_and_fitting_fields_compare_by_value() {
    // Değerler elle: x = P { a: 1, s: -1, b: true } yazılır, bir çevrim
    // sonra q aynıdır (q = r, r <= x).
    //  - "right value" ve "computed right value": q ile aynı -> geçer.
    //    İşaretli alan 0 - 1 (i4 -1, desen 0xF) geçerli bir alan değeri.
    //  - "wrong value": a = 2 sığar ama q.a = 1 -> düşer (differs: a).
    //  - "computed value too wide": a = 17 u4'e sığmaz -> koşuda düşer.
    //    Önceden 17 & 15 = 1 = q.a olduğu için geçiyordu.
    let tests = "
test \"right value\" {
    let dut = M { };
    dut.x = P { a: 1, s: 0 - 1, b: true };
    step(1);
    assert_eq(dut.q, P { a: 1, s: 0 - 1, b: true });
}

test \"computed right value\" {
    let dut = M { };
    dut.x = P { a: 1, s: 0 - 1, b: true };
    step(1);
    for i in 1..2 {
        assert_eq(dut.q, P { a: i, s: 0 - 1, b: true });
    }
}

test \"wrong value\" {
    let dut = M { };
    dut.x = P { a: 1, s: 0 - 1, b: true };
    step(1);
    assert_eq(dut.q, P { a: 2, s: 0 - 1, b: true });
}

test \"computed value too wide\" {
    let dut = M { };
    dut.x = P { a: 1, s: 0 - 1, b: true };
    step(1);
    for i in 17..18 {
        assert_eq(dut.q, P { a: i, s: 0 - 1, b: true });
    }
}
";
    let Some((out, all)) = run("computed", tests) else {
        return;
    };
    assert_eq!(out.status.code(), Some(5), "{all}");
    assert!(all.contains("test right_value ... ok"), "{all}");
    assert!(all.contains("test computed_right_value ... ok"), "{all}");
    assert!(all.contains("test wrong_value ... FAILED"), "{all}");
    assert!(all.contains("differs: a"), "{all}");
    assert!(
        all.contains("test computed_value_too_wide ... FAILED"),
        "{all}"
    );
    assert!(all.contains("cannot hold value 17"), "{all}");
    assert!(
        all.contains("test result: FAILED. 2 passed; 2 failed"),
        "{all}"
    );
}

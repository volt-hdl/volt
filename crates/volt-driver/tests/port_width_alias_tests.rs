//! Takma ad ya da const ifadesiyle verilen port genişliği test dilinde de
//! bilinir (#376) — uçtan uca, gerçek Verilator'da.
//!
//! Önceden test dili genişliği yalnız düz literalden (`u4`, `uint<4>`) ve
//! düz literal değerli `const`'tan çözüyordu; `type Nib = u4` ve
//! `const W : u32 = 2 + 2` "bilinmiyor" sayılıyordu. Sabit değer E8512'den
//! kaçıyor, hesaplanmış değer yalnız C++ depolama tipine (8 bit) göre
//! denetleniyordu: `dut.a = 31` 4 bitlik porta ulaşıyor, Verilator girişi
//! maskelemediğinden 4 bitlik `q` 31 okuyordu (ADR-0059 §1-§4).
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::{Command, Output};

const DESIGN: &str = "\
type Nib = u4
const W : u32 = 2 + 2

module M {
    in  clk : clock
    in  a   : Nib
    in  b   : uint<W>
    out q   : Nib
    out qb  : uint<W>

    reg r  : Nib = 0
    reg rb : uint<W> = 0
    on clk {
        r <= a
        rb <= b
    }
    q = r
    qb = rb
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-alias-width-{tag}-{}", std::process::id()));
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
fn issue_376_a_constant_that_does_not_fit_an_aliased_port_is_rejected() {
    // #376'nın örneği: 31, `Nib` (u4) portuna sığmaz; 4 bitlik `q` 31
    // okuyamaz. Önceden ikisi de kabul ediliyor ve test geçiyordu.
    let tests = "
test \"alias write\" {
    let dut = M { };
    dut.a = 31;
    step(1);
    assert_eq(dut.q, 31);
}
";
    let Some((out, all)) = run("const", tests) else {
        return;
    };
    assert_eq!(out.status.code(), Some(1), "{all}");
    assert_eq!(all.matches("error[E8512]").count(), 2, "{all}");
    assert!(!all.contains("passed"), "{all}");
}

#[test]
fn a_computed_value_that_does_not_fit_fails_the_test_and_fitting_values_pass() {
    // Değerler elle: r <= a, q = r; bir çevrim sonra q yazılan değerdir.
    //  - "right values": 5 ve 9 dört bite sığar -> geçer.
    //  - "too wide for the alias" / "... for the const width": 31, 4 bitlik
    //    porta sığmaz -> koşuda port_overflow ile düşer. Önceden 31 porta
    //    ulaşıyor, q 31 okuyor ve test geçiyordu.
    let tests = "
test \"right values\" {
    let dut = M { };
    for i in 5..6 {
        dut.a = i;
        dut.b = i + 4;
    }
    step(1);
    assert_eq(dut.q, 5);
    assert_eq(dut.qb, 9);
}

test \"too wide for the alias\" {
    let dut = M { };
    for i in 31..32 {
        dut.a = i;
        step(1);
        assert_eq(dut.q, i);
    }
}

test \"too wide for the const width\" {
    let dut = M { };
    for i in 31..32 {
        dut.b = i;
        step(1);
        assert_eq(dut.qb, i);
    }
}
";
    let Some((out, all)) = run("computed", tests) else {
        return;
    };
    assert_eq!(out.status.code(), Some(5), "{all}");
    assert!(all.contains("test right_values ... ok"), "{all}");
    assert!(
        all.contains("test too_wide_for_the_alias ... FAILED"),
        "{all}"
    );
    assert!(
        all.contains("test too_wide_for_the_const_width ... FAILED"),
        "{all}"
    );
    assert!(all.contains("port 'a' (u4) cannot hold value 31"), "{all}");
    assert!(all.contains("port 'b' (u4) cannot hold value 31"), "{all}");
    assert!(
        all.contains("test result: FAILED. 1 passed; 2 failed"),
        "{all}"
    );
}

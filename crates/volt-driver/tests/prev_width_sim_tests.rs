//! `prev(x)`'in değeri `volt test`'te `x`'in tipindedir — uçtan uca, gerçek
//! Verilator'da (#413, ADR-0040: `prev(x: T) -> T`). Önceden yardımcı
//! register üreticinin kendi genişliğiyle ve her zaman işaretsiz
//! bildiriliyordu: işaretli `-1` 255 olarak karşılaştırılıyor, `u8 + u8`'in
//! 300'ü 44'e kesiliyordu.
//!
//! Her tasarımda iki kontrat vardır: biri Volt'ta tutar, öteki ihlal edilir.
//! Test ikisini de ister; yanlış işaret ya da genişlik ikisinin yerini
//! değiştirir (işaretsiz karşılaştırmada `-1 < 0` ihlal, `-1 > 0` tutar).
//! Değerler spec'ten elle hesaplandı (yorumlarda).
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-prevwidth-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt test` çıktısı (stdout + stderr).
fn volt_test(tag: &str, src: &str) -> String {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("prev_width_test.volt"), src).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["test", "prev_width_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let _ = std::fs::remove_dir_all(&dir);
    format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// `held` testi geçer, `violated` testi `contract` kontratının ihlaliyle
/// düşer.
fn assert_one_holds_one_violated(out: &str, held: &str, violated: &str, contract: &str) {
    assert!(
        out.contains("test result: FAILED. 1 passed; 1 failed"),
        "{out}"
    );
    assert!(out.contains(&format!("test {held} ... ok")), "{out}");
    assert!(
        out.contains(&format!("test {violated} ... FAILED")),
        "{out}"
    );
    assert!(out.contains(contract), "{out}");
}

/// `sa` ve `sb` i8; test dili işaretli değeri bit deseniyle yazar
/// (ADR-0059): 255 = -1.
const SIGNED: &str = "\
module Less {
    in  clk : clock
    in  chk : bool
    in  sa  : i8
    in  sb  : i8
    out y   : i8
    reg r : i8 = 0
    on clk { r <= sa }
    y = r
    invariant: !chk || prev(sa) < sb
}

module Greater {
    in  clk : clock
    in  chk : bool
    in  sa  : i8
    in  sb  : i8
    out y   : i8
    reg r : i8 = 0
    on clk { r <= sa }
    y = r
    invariant: !chk || prev(sa) > sb
}

test \"less holds\" {
    let dut = Less { };
    dut.sa = 255;
    dut.sb = 0;
    step(3);
    dut.chk = true;
    step(2);
}

test \"greater is violated\" {
    let dut = Greater { };
    dut.sa = 255;
    dut.sb = 0;
    step(3);
    dut.chk = true;
    step(2);
}
";

/// prev(sa) = -1 (bir çevrim önceki sa), sb = 0: `-1 < 0` doğru, `-1 > 0`
/// yanlış (i8 karşılaştırması işaretlidir, type-inference.md §3).
#[test]
fn prev_of_a_signed_value_compares_signed() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let out = volt_test("signed", SIGNED);
    assert_one_holds_one_violated(
        &out,
        "less_holds",
        "greater_is_violated",
        "invariant: !chk || prev(sa) > sb",
    );
}

const SUM: &str = "\
module Equal {
    in  clk : clock
    in  chk : bool
    in  a   : u8
    in  b   : u8
    in  c   : u9
    out y   : u8
    reg r : u8 = 0
    on clk { r <= a }
    y = r
    invariant: !chk || prev(a + b) == c
}

module Differ {
    in  clk : clock
    in  chk : bool
    in  a   : u8
    in  b   : u8
    in  c   : u9
    out y   : u8
    reg r : u8 = 0
    on clk { r <= a }
    y = r
    invariant: !chk || prev(a + b) != c
}

test \"equal holds\" {
    let dut = Equal { };
    dut.a = 200;
    dut.b = 100;
    dut.c = 300;
    step(3);
    dut.chk = true;
    step(2);
}

test \"differ is violated\" {
    let dut = Differ { };
    dut.a = 200;
    dut.b = 100;
    dut.c = 300;
    step(3);
    dut.chk = true;
    step(2);
}
";

/// a + b = 300: u8 + u8'in doğal genişliği 9 bit (ADR-0025,
/// type-inference.md §3.3), prev(a + b) de 9 bittir. c = 300 iken
/// `prev(a + b) == c` doğru, `!=` yanlış; 8 bitte 44 iki sonucu da
/// tersine çevirirdi.
#[test]
fn prev_of_a_sum_keeps_its_carry() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let out = volt_test("sum", SUM);
    assert_one_holds_one_violated(
        &out,
        "equal_holds",
        "differ_is_violated",
        "invariant: !chk || prev(a + b) != c",
    );
}

//! Bileşik değerden azalan indexed part-select (`v[i -: W]`), başlangıcın
//! doğal genişliği 126, 127, 128 ve 129 bitken (#282), uçtan uca: `volt
//! check` tanısız, `volt test` gerçek Verilator'da elle hesaplanan değeri
//! okur.
//!
//! Üretici (`part_select_from_value`) değişken başlangıçtan `W - 1`'i
//! çıkarır; sabitin genişliği başlangıcın genişliğine göre seçilir ve
//! seçimde 127 bit bir sınırdır (`sig.width < 127`). uN/iN 64 bitte biter
//! (E0003) ve iki operand aynı genişlikte olmalıdır (E2001): başlangıç 64
//! bitin üstüne yalnız bileşik ifadeyle çıkar. `s * t` (u63) 126, `s * t +
//! 1` 127, `u * v` (u64) 128, `u * v + 1` 129 bittir; işaretlide `si * ti +
//! (-1)` (i63) 127, `x * y` (i64) 128 bittir.
//!
//! Şartname: ADR-0035, `x[i -: W]` i..i-W+1 bitlerini okur; bileşik değerin
//! seçimi değerin doğal genişliğinde hesaplanır (#112, `part_select_value_tests`).
//! `a * b` (u64 · u64) 128 bittir. Beklenen değerler girişlerden elle
//! hesaplandı (yorumlarda), derleyici çıktısından alınmadı; üretilen SV'nin
//! metni karşılaştırılmaz. `volt test` Verilator'ın WIDTH uyarılarında da
//! durur: geçen test lint temizdir.
//!
//! Test dili negatif literal almaz: işaretli girişler ikiye tümleyen bit
//! örüntüsüyle yazılır (i63'te -5 = 2^63 - 5 = 0x7FFF_FFFF_FFFF_FFFB).
//!
//! Verilator yoksa simülasyon testi atlanır; `VOLT_REQUIRE_TOOLS=verilator`
//! ile (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DESIGN: &str = "\
module WideStart {
    in  a    : u64
    in  b    : u64
    in  s    : u63
    in  t    : u63
    in  u    : u64
    in  v    : u64
    in  si   : i63
    in  ti   : i63
    in  x    : i64
    in  y    : i64
    out w126 : bits<8>
    out w127 : bits<8>
    out w128 : bits<8>
    out w129 : bits<8>
    out n127 : bits<8>
    out n128 : bits<8>

    w126 = (a * b)[(s * t) -: 8]
    w127 = (a * b)[(s * t + 1) -: 8]
    w128 = (a * b)[(u * v) -: 8]
    w129 = (a * b)[(u * v + 1) -: 8]
    n127 = (a * b)[(si * ti + (-1)) -: 8]
    n128 = (a * b)[(x * y) -: 8]
}
";

// 1. blok: a * b = a = 0x7EDC_BA98_7654_3210; bayt k (bit 8k+7..8k):
// 10 32 54 76 98 BA DC 7E (k = 0..7).
// - w126: 3 · 5 = 15, bit 15..8 = bayt 1 = 0x32.
// - w127: 15 + 1 = 16, bit 16..9: bit 16 (0x54'ün en alt biti) 0, bit
//   15..9 0x32'nin (0011 0010) üst yedi biti 0011001: 0001_1001 = 0x19.
// - w128: 7 · 9 = 63, bit 63..56 = bayt 7 = 0x7E.
// - w129: 63 + 1 = 64, bit 64 (a * b 64 bitin üstünde 0) ve bit 63..57
//   (0x7E = 0111 1110'nun üst yedi biti 0111111): 0011_1111 = 0x3F.
// - n127: (-5) · (-5) + (-1) = 24, bit 24..17: (0x7654_3210 >> 17) =
//   0x3B2A, alt bayt 0x2A.
// - n128: (-3) · (-7) = 21, bit 21..14: 0x54_3210 = 0101 0100 0011 0010
//   ...; bit 21..16 = 010100, bit 15..14 = 00: 0101_0000 = 0x50.
//
// 2. blok: a * b = a · 2^32 = 0x0123_4567_89AB_CDEF << 32; çarpımın
// i. biti a'nın (i - 32). bitidir.
// - w126: 10 · 9 = 90, a'nın bit 58..51'i: a >> 48 = 0x0123, >> 3 = 0x24.
// - w127: 91, a'nın bit 59..52'si: 0x0123 >> 4 = 0x12.
// - w128: 8 · 10 = 80, a'nın bit 48..41'i: a >> 40 = 0x01_2345, >> 1 =
//   0x91A2, alt bayt 0xA2.
// - w129: 81, a'nın bit 49..42'si: 0x01_2345 >> 2 = 0x48D1, alt bayt 0xD1.
// - n127: (-7) · (-7) + (-1) = 48, a'nın bit 16..9'u: 0xAB_CDEF >> 8 =
//   0xABCD, >> 1 = 0x55E6, alt bayt 0xE6.
// - n128: (-6) · (-11) = 66, a'nın bit 34..27'si: a'nın bit 39..24'ü
//   0x6789, >> 3 = 0xCF1, alt bayt 0xF1.
const TESTS: &str = "\
test \"starts below 64\" {
    let dut = WideStart { };
    dut.a = 0x7EDC_BA98_7654_3210;
    dut.b = 1;
    dut.s = 3;
    dut.t = 5;
    dut.u = 7;
    dut.v = 9;
    dut.si = 0x7FFF_FFFF_FFFF_FFFB;
    dut.ti = 0x7FFF_FFFF_FFFF_FFFB;
    dut.x = 0xFFFF_FFFF_FFFF_FFFD;
    dut.y = 0xFFFF_FFFF_FFFF_FFF9;
    step(1);
    assert_eq(dut.w126, 0x32);
    assert_eq(dut.w127, 0x19);
    assert_eq(dut.w128, 0x7E);
    assert_eq(dut.w129, 0x3F);
    assert_eq(dut.n127, 0x2A);
    assert_eq(dut.n128, 0x50);
}

test \"starts above 64\" {
    let dut = WideStart { };
    dut.a = 0x0123_4567_89AB_CDEF;
    dut.b = 0x1_0000_0000;
    dut.s = 10;
    dut.t = 9;
    dut.u = 8;
    dut.v = 10;
    dut.si = 0x7FFF_FFFF_FFFF_FFF9;
    dut.ti = 0x7FFF_FFFF_FFFF_FFF9;
    dut.x = 0xFFFF_FFFF_FFFF_FFFA;
    dut.y = 0xFFFF_FFFF_FFFF_FFF5;
    step(1);
    assert_eq(dut.w126, 0x24);
    assert_eq(dut.w127, 0x12);
    assert_eq(dut.w128, 0xA2);
    assert_eq(dut.w129, 0xD1);
    assert_eq(dut.n127, 0xE6);
    assert_eq(dut.n128, 0xF1);
}
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-psel-wide-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    std::fs::write(dir.join("wide_start.volt"), DESIGN).expect("yaz");
    std::fs::write(dir.join("wide_start_test.volt"), TESTS).expect("yaz");
    dir
}

fn volt(dir: &Path, args: &[&str]) -> (Output, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("volt");
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out, text)
}

/// `volt check` ve `volt build` sınırın iki yanındaki her başlangıç
/// genişliğinde tanı vermez ve başarıyla biter. Araç gerektirmez.
#[test]
fn issue_282_wide_starts_check_and_build_without_diagnostics() {
    let dir = temp_dir("check");
    for args in [
        &["check", "wide_start.volt"][..],
        &["check", "wide_start_test.volt"][..],
        &["build", "wide_start.volt", "--target-dir", "build"][..],
    ] {
        let (out, text) = volt(&dir, args);
        assert!(out.status.success(), "{args:?}\n{text}");
        assert!(
            !text.contains("error[") && !text.contains("warning["),
            "{args:?}\n{text}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Her başlangıç genişliğinde seçim, Verilator'da elle hesaplanan bitleri
/// okur.
#[test]
fn issue_282_wide_starts_select_in_simulation() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("sim");
    let (out, text) = volt(
        &dir,
        &["test", "wide_start_test.volt", "--target-dir", "build"],
    );
    // CI günlüğünde testin gerçekten Verilator'da koştuğu görünsün
    // (atlanan test de "ok" yazar).
    eprintln!("--- volt test (part_select_wide_start_tests)\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(text.contains("2 passed; 0 failed"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

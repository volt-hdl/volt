//! 2^127 ve üstündeki tam sayı literalleri değerleriyle yazılır (#207),
//! davranış olarak, gerçek Verilator'da.
//!
//! Şartname: ast-nodes.md'de `IntLit { value: u128, .. }`: literalin
//! değeri 128 bitlik işaretsiz sayıdır, 2^128 - 1'e kadar her değer
//! geçerlidir (daha genişi E0005, `tests/ui/fail/288`). `x as bits<N>`
//! (type-inference.md §3.6, "Literal açık dönüşümle her sayısal tipe
//! gider") N bite sığan literalin değerini taşır. Düzeltmeden önce üretici
//! değeri `i128`'e çevirirken sığmayanı `i128::MAX`'e doyuruyordu: 2^127,
//! 2^127 + 1 ve 2^128 - 1'in üçü de `128'h7FFF…FFFF` yazılıyordu (reset
//! değeri, maske, karşılaştırma).
//!
//! Test dili 64 bitlik değerler taşır: 128 ve 256 bitlik değerler 64
//! bitlik dilimlerle okunur (`r[127:64]`), girişler 64 bitlik iki porttan
//! struct dönüşümüyle (`W { hi, lo } as bits<128>`, ADR-0077 Karar 5)
//! kurulur. Beklenen değerler literalden elle yazıldı (yorumlarda),
//! derleyici çıktısından alınmadı. Üretilen SV'nin metni karşılaştırılmaz;
//! `volt test` Verilator'ın WIDTH uyarılarında da durur, geçen test lint
//! temizdir.
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-wide-lit-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `src`'yi (tasarım + test blokları) `volt test` ile koşar; `passed` test
/// geçmeli, hiçbiri düşmemeli.
fn sim(tag: &str, src: &str, passed: usize) {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir(tag);
    std::fs::write(dir.join("case.volt"), src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["test", "case.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // CI günlüğünde testin Verilator'da koştuğu görünsün (atlanan test de
    // "ok" yazar).
    eprintln!("--- volt test (wide_literal_sim_tests::{tag})\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 128 bitlik `K` (Volt literal metni) bir register'ın reset değeri, bir
/// maske ve bir karşılaştırmanın sabiti olarak; `hi`/`lo` K'nın üst ve alt
/// 64 biti (elle), `near_hi`/`near_lo` K'dan farklı bir değer (doyurulmuş
/// yazım `2^127 - 1`). Üç test: reset değeri, maske, eşitlik.
fn wide128(tag: &str, k: &str, hi: &str, lo: &str, near_hi: &str, near_lo: &str) {
    let src = format!(
        r#"struct W {{ hi: bits<64>, lo: bits<64> }}
module Wide {{
    in  clk  : clock
    in  dh   : bits<64>
    in  dl   : bits<64>
    out r_hi : bits<64>
    out r_lo : bits<64>
    out m_hi : bits<64>
    out m_lo : bits<64>
    out eq   : bool
    out ne   : bool
    reg r : bits<128> = {k} as bits<128>
    let w : W = W {{ hi: dh, lo: dl }}
    let d : bits<128> = w as bits<128>
    on clk {{ r <= r ^ d }}
    let m : bits<128> = d & ({k} as bits<128>)
    r_hi = r[127:64]
    r_lo = r[63:0]
    m_hi = m[127:64]
    m_lo = m[63:0]
    eq = d == ({k} as bits<128>)
    ne = d != ({k} as bits<128>)
}}
test "reset value" {{
    let dut = Wide {{ }};
    dut.dh = 0;
    dut.dl = 0;
    step(1);
    assert_eq(dut.r_hi, {hi});
    assert_eq(dut.r_lo, {lo});
}}
test "mask" {{
    let dut = Wide {{ }};
    dut.dh = 0xFFFF_FFFF_FFFF_FFFF;
    dut.dl = 0xFFFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.m_hi, {hi});
    assert_eq(dut.m_lo, {lo});
}}
test "comparison" {{
    let dut = Wide {{ }};
    dut.dh = {hi};
    dut.dl = {lo};
    step(1);
    assert_eq(dut.eq, true);
    assert_eq(dut.ne, false);
    dut.dh = {near_hi};
    dut.dl = {near_lo};
    step(1);
    assert_eq(dut.eq, false);
    assert_eq(dut.ne, true);
}}
"#
    );
    sim(tag, &src, 3);
}

/// 2^127 = 0x8000_0000_0000_0000_0000_0000_0000_0000, ondalık yazılmış:
/// üst 64 bit 0x8000_0000_0000_0000, alt 64 bit 0. Düzeltmeden önce
/// `128'd170141183460469231731687303715884105727` (2^127 - 1).
#[test]
fn issue_207_two_pow_127_in_reset_mask_and_comparison() {
    wide128(
        "p127",
        "170141183460469231731687303715884105728",
        "0x8000_0000_0000_0000",
        "0",
        "0x7FFF_FFFF_FFFF_FFFF",
        "0xFFFF_FFFF_FFFF_FFFF",
    );
}

/// 2^127 + 1, onaltılık: üst 64 bit 0x8000_0000_0000_0000, alt 64 bit 1.
/// Karşılaştırmanın ikinci değeri 2^127 (yalnız en alt bit farklı).
#[test]
fn issue_207_two_pow_127_plus_one_in_reset_mask_and_comparison() {
    wide128(
        "p127p1",
        "0x8000_0000_0000_0000_0000_0000_0000_0001",
        "0x8000_0000_0000_0000",
        "1",
        "0x8000_0000_0000_0000",
        "0",
    );
}

/// 2^128 - 1, onaltılık: iki yarı da 0xFFFF_FFFF_FFFF_FFFF. Karşılaştırmanın
/// ikinci değeri doyurulmuş yazım 2^127 - 1.
#[test]
fn issue_207_two_pow_128_minus_one_in_reset_mask_and_comparison() {
    wide128(
        "p128m1",
        "0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF",
        "0xFFFF_FFFF_FFFF_FFFF",
        "0xFFFF_FFFF_FFFF_FFFF",
        "0x7FFF_FFFF_FFFF_FFFF",
        "0xFFFF_FFFF_FFFF_FFFF",
    );
}

/// Issue'nun örneği: 0x8000…0001 reset değeri ve 0xDEAD_BEEF_0123_4567_
/// 89AB_CDEF_0000_0001 karşılaştırması; bu kez ikili ve sekizli tabanlar
/// da: 2^128 - 1'in ikili (128 bir) ve sekizli (`0o3` ve 42 tane `7`:
/// 2 + 42·3 = 128 bit) yazımı aynı değerdir.
#[test]
fn issue_207_every_base_writes_the_same_wide_value() {
    let ones = "1".repeat(128);
    let octal = format!("0o3{}", "7".repeat(42));
    let src = format!(
        r#"struct W {{ hi: bits<64>, lo: bits<64> }}
module Bases {{
    in  dh  : bits<64>
    in  dl  : bits<64>
    out bin : bool
    out oct : bool
    out dec : bool
    out key : bool
    let w : W = W {{ hi: dh, lo: dl }}
    let d : bits<128> = w as bits<128>
    bin = d == (0b{ones} as bits<128>)
    oct = d == ({octal} as bits<128>)
    dec = d == (340282366920938463463374607431768211455 as bits<128>)
    key = d == (0xDEAD_BEEF_0123_4567_89AB_CDEF_0000_0001 as bits<128>)
}}
test "all ones in every base" {{
    let dut = Bases {{ }};
    dut.dh = 0xFFFF_FFFF_FFFF_FFFF;
    dut.dl = 0xFFFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.bin, true);
    assert_eq(dut.oct, true);
    assert_eq(dut.dec, true);
    assert_eq(dut.key, false);
    dut.dh = 0x7FFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.bin, false);
    assert_eq(dut.oct, false);
    assert_eq(dut.dec, false);
}}
test "the issue's key" {{
    let dut = Bases {{ }};
    dut.dh = 0xDEAD_BEEF_0123_4567;
    dut.dl = 0x89AB_CDEF_0000_0001;
    step(1);
    assert_eq(dut.key, true);
    assert_eq(dut.dec, false);
}}
"#
    );
    sim("bases", &src, 2);
}

/// 128 bitten geniş bağlam: 2^128 - 1 ve 2^127 `bits<256>`'da. Literal
/// 128 bite sığar, üst 128 bit sıfırla dolar: dilimler (üstten alta) 0, 0,
/// 0xFFFF_FFFF_FFFF_FFFF, 0xFFFF_FFFF_FFFF_FFFF; 2^127'de 0, 0,
/// 0x8000_0000_0000_0000, 0. Düzeltmeden önce
/// `256'h7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF`.
#[test]
fn issue_207_wide_literal_in_a_256_bit_context() {
    let src = r#"struct Q { a: bits<64>, b: bits<64>, c: bits<64>, d: bits<64> }
module Wide256 {
    in  clk : clock
    in  q3  : bits<64>
    in  q2  : bits<64>
    in  q1  : bits<64>
    in  q0  : bits<64>
    out r3  : bits<64>
    out r2  : bits<64>
    out r1  : bits<64>
    out r0  : bits<64>
    out m1  : bits<64>
    out m0  : bits<64>
    out eq  : bool
    reg r : bits<256> = 0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF as bits<256>
    let q : Q = Q { a: q3, b: q2, c: q1, d: q0 }
    let x : bits<256> = q as bits<256>
    on clk { r <= r ^ x }
    let m : bits<256> = x & (170141183460469231731687303715884105728 as bits<256>)
    r3 = r[255:192]
    r2 = r[191:128]
    r1 = r[127:64]
    r0 = r[63:0]
    m1 = m[127:64]
    m0 = m[63:0]
    eq = x == (0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF as bits<256>)
}
test "reset value, mask and comparison in bits<256>" {
    let dut = Wide256 { };
    dut.q3 = 0;
    dut.q2 = 0;
    dut.q1 = 0;
    dut.q0 = 0;
    step(1);
    assert_eq(dut.r3, 0);
    assert_eq(dut.r2, 0);
    assert_eq(dut.r1, 0xFFFF_FFFF_FFFF_FFFF);
    assert_eq(dut.r0, 0xFFFF_FFFF_FFFF_FFFF);
    assert_eq(dut.eq, false);
    dut.q1 = 0xFFFF_FFFF_FFFF_FFFF;
    dut.q0 = 0xFFFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.m1, 0x8000_0000_0000_0000);
    assert_eq(dut.m0, 0);
    assert_eq(dut.eq, true);
    dut.q2 = 1;
    step(1);
    assert_eq(dut.eq, false);
}
"#;
    sim("w256", src, 1);
}

/// Denetim, işaretli taraf: tam sayılar en çok 64 bittir (`i128` E0003),
/// işaretli literalin sınırları i64'ünkilerdir. -2^63 onaltılık ve ondalık,
/// 2^63 - 1; karşılaştırmada da. Test dili işaretsizdir: değerler aynı
/// genişlikte `as u64` ile bit örüntüsü olarak okunur (ADR-0036):
/// -2^63 → 0x8000_0000_0000_0000, 2^63 - 1 → 0x7FFF_FFFF_FFFF_FFFF.
/// Düzeltmeden önce de geçiyordu; işaret-büyüklük yazımı bu sınırları
/// korumalı.
#[test]
fn issue_207_signed_boundaries_keep_their_value() {
    let src = r#"module Signed {
    in  s      : i64
    out mn_hex : u64
    out mn_dec : u64
    out mx     : u64
    out lt     : bool
    out eq     : bool
    let a : i64 = -0x8000_0000_0000_0000
    let b : i64 = -9223372036854775808
    let c : i64 = 0x7FFF_FFFF_FFFF_FFFF
    mn_hex = a as u64
    mn_dec = b as u64
    mx = c as u64
    lt = s < -0x7FFF_FFFF_FFFF_FFFF
    eq = s == -9223372036854775808
}
test "i64 boundaries" {
    let dut = Signed { };
    dut.s = 0x8000_0000_0000_0000;
    step(1);
    assert_eq(dut.mn_hex, 0x8000_0000_0000_0000);
    assert_eq(dut.mn_dec, 0x8000_0000_0000_0000);
    assert_eq(dut.mx, 0x7FFF_FFFF_FFFF_FFFF);
    assert_eq(dut.lt, true);
    assert_eq(dut.eq, true);
    dut.s = 0x8000_0000_0000_0001;
    step(1);
    assert_eq(dut.lt, false);
    assert_eq(dut.eq, false);
}
"#;
    sim("signed", src, 1);
}

/// Derleme zamanı ile çalışma zamanı aynı değeri görür, i128 sınırında:
/// `K = (2^127 - 1) / 2^63` = 2^64 - 1 (const-eval.md §4, sıfıra doğru;
/// 2^127 - 1 = 2^63 · (2^64 - 1) + 2^63 - 1). Değerlendirici K'yı
/// `match`'in kolunda kullanır: `0xFFFF_FFFF_FFFF_FFFF` kolu K'nın
/// kolundan sonra gelir ve hiç seçilmez (W2014, `issue_207_const_and_
/// runtime_see_the_same_value_at_compile_time`). Donanım K'yı çıkışa
/// yazar ve `x = 2^64 - 1` için K'nın kolunu seçer.
const CONST_RUNTIME: &str = r#"const K : u64 = 0x7FFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF / 0x8000_0000_0000_0000
module ConstRun {
    in  x : u64
    out k : u64
    out y : u8
    k = K
    y = match x {
        K => 1,
        0xFFFF_FFFF_FFFF_FFFF => 2,
        _ => 3,
    }
}
"#;

#[test]
fn issue_207_const_and_runtime_see_the_same_value_in_hardware() {
    let src = format!(
        r#"{CONST_RUNTIME}
test "K is 2^64 - 1 in hardware" {{
    let dut = ConstRun {{ }};
    dut.x = 0xFFFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.k, 0xFFFF_FFFF_FFFF_FFFF);
    assert_eq(dut.y, 1);
    dut.x = 0xFFFF_FFFF_FFFF_FFFE;
    step(1);
    assert_eq(dut.y, 3);
}}
"#
    );
    sim("construn", &src, 1);
}

/// Aynı tasarımın derleme zamanı yanı: değerlendirici K'yı 2^64 - 1
/// bulduğu için ikinci kolu erişilmez bulur (W2014), donanımda da ikinci
/// kol seçilmez (yukarıdaki test). Araç gerektirmez.
#[test]
fn issue_207_const_and_runtime_see_the_same_value_at_compile_time() {
    let dir = temp_dir("constw");
    std::fs::write(dir.join("case.volt"), CONST_RUNTIME).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["build", "case.volt", "--target-dir", "build"])
        .output()
        .expect("volt build");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{text}");
    assert!(text.contains("warning[W2014]"), "{text}");
    assert!(
        text.contains("0 error(s)") || !text.contains("error["),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Değerlendiricinin aralığı `i128`'dir (const-eval.md §1); dışındaki
/// literal taşmadır (§4, E2022), sarılmaz. `(2^128 - 1) / (2^64 + 1)` =
/// 2^64 - 1 matematikte u64'e sığar, ama 2^128 - 1 aralıkta değildir.
/// Düzeltmeden önce değerlendirici literali -1'e sarıyordu: derleme
/// zamanı K = 0 görüyor, üretici aynı sabiti katlayamıyordu (E0003).
#[test]
fn issue_207_const_literal_beyond_i128_is_an_overflow() {
    let src = r#"const K : u64 = 0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF / 0x1_0000_0000_0000_0001
module ConstWrap {
    out k : u64
    k = K
}
"#;
    let dir = temp_dir("constwrap");
    std::fs::write(dir.join("case.volt"), src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["build", "case.volt", "--target-dir", "build"])
        .output()
        .expect("volt build");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.status.success(), "{text}");
    assert!(text.contains("error[E2022]"), "{text}");
    assert!(!text.contains("E0003"), "{text}");
    assert!(!dir.join("build/rtl").exists(), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `tests/ui/fail/288` (E0005, 2^128 `bits<256>`'da), `275` (E2022, const'ta
/// i128 dışı literal) ve `276` (W2010, sığmayan literalin dönüşümü,
/// ADR-0115), `277`–`284` (#213, sığmayan negatif literal: issue'nun her
/// satırı), `285` ve `286` (#262, parametre tipine sığmayan generic argüman;
/// 286'da genişlik önceki parametre): her
/// biri tam kendi kodunu, beklenen satırda ve tek tanı olarak verir.
/// Araç gerektirmez.
#[test]
fn issue_207_ui_fail_fixtures_report_their_code() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ui/fail");
    for (name, code, line) in [
        ("288_literal_wider_than_128_bits.volt", "E0005", 7),
        ("275_const_literal_beyond_i128.volt", "E2022", 5),
        ("276_literal_cast_cuts_w2010.volt", "W2010", 7),
        // #213: negatif literal hedefinin aralığına sığmalı.
        ("277_negative_literal_out_of_range_let.volt", "E2010", 5),
        (
            "278_negative_literal_out_of_range_comparison.volt",
            "E2010",
            7,
        ),
        ("279_negative_literal_out_of_range_reset.volt", "E2010", 6),
        (
            "280_negative_literal_out_of_range_register_write.volt",
            "E2010",
            7,
        ),
        ("281_negative_literal_out_of_range_const.volt", "E2010", 3),
        ("282_negative_literal_out_of_range_operand.volt", "E2010", 7),
        ("283_negative_literal_unsigned_target.volt", "E2010", 5),
        ("284_negative_literal_pattern_out_of_range.volt", "E2010", 8),
        // #262: generic argüman parametrenin tipine sığmalı.
        ("285_generic_argument_out_of_range.volt", "E2010", 10),
        (
            "286_generic_argument_out_of_range_parameter_width.volt",
            "E2010",
            10,
        ),
        // #272: 127 bit ve üstü bağlamda negatif literal.
        ("287_negative_literal_in_a_128_bit_context.volt", "E2010", 8),
    ] {
        let src = std::fs::read_to_string(root.join(name)).expect("fikstür");
        assert!(src.starts_with(&format!("//~ {code}")), "{name}");
        let dir = temp_dir(&name[..3]);
        std::fs::write(dir.join("case.volt"), &src).expect("yaz");
        let out = Command::new(env!("CARGO_BIN_EXE_volt"))
            .env("VOLT_LANG", "en")
            .env("NO_COLOR", "1")
            .current_dir(&dir)
            .args(["build", "case.volt", "--target-dir", "build"])
            .output()
            .expect("volt build");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        // Uyarı fikstürü derlenir, tek tanısı uyarıdır; hata fikstürü
        // tek hatayla durur.
        let (kind, ok) = if code.starts_with('W') {
            ("warning", true)
        } else {
            ("error", false)
        };
        assert_eq!(out.status.success(), ok, "{name}: {text}");
        assert_eq!(
            text.matches("error[").count(),
            usize::from(!ok),
            "{name}: {text}"
        );
        assert_eq!(
            text.matches(&format!("{kind}[")).count(),
            1,
            "{name}: {text}"
        );
        assert!(text.contains(&format!("{kind}[{code}]")), "{name}: {text}");
        assert!(
            text.contains(&format!("case.volt:{line}:")),
            "{name}: {text}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// E2005'in iletisi literalin değerini yazar: 2^127 + 1 ve -2^127 de
/// değerleriyle (iki literalin karşılaştırmasının genişliği belirsizdir).
/// Düzeltmeden önce değer doyuruluyordu: `-2^127` için ileti
/// `'-170141183460469231731687303715884105727'` diyordu. Araç gerektirmez.
#[test]
fn issue_207_width_diagnostic_shows_the_literal_value() {
    let src = "module M {
    out o : bool
    out p : bool
    o = 0x8000_0000_0000_0000_0000_0000_0000_0001 == 5
    p = -170141183460469231731687303715884105728 == -5
}
";
    let dir = temp_dir("e2005");
    std::fs::write(dir.join("case.volt"), src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["build", "case.volt", "--target-dir", "build"])
        .output()
        .expect("volt build");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.status.success(), "{text}");
    for value in [
        "'170141183460469231731687303715884105729'",
        "'5'",
        "'-170141183460469231731687303715884105728'",
        "'-5'",
    ] {
        assert!(
            text.contains(&format!(
                "error[E2005]: cannot determine the width of the literal {value}"
            )),
            "{value}: {text}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Bağlamdan geniş pozitif literal kesilmiş örüntüsüyle yazılır (#123;
/// `fmt_int`'in #207'de yeniden yazılan dalı). `(a + b)` esnektir (u1 +
/// u1, doğal genişlik 2), literal ona sığar (A9) ama işlem u1 bağlamında
/// yazılır. Değer iki okumada da (doğal genişlikte ya da u1'de) aynıdır,
/// elle: `((a + b) | 3) & c` = c (3'ün alt biti 1), `((a + b) | 2) & c`
/// = (a ^ b) & c (2'nin alt biti 0, a + b'nin alt biti a ^ b).
#[test]
fn issue_207_literal_wider_than_its_context_keeps_its_low_bits() {
    let src = r#"module Cut {
    in  a  : u1
    in  b  : u1
    in  c  : u1
    out o3 : u1
    out o2 : u1
    o3 = ((a + b) | 3) & c
    o2 = ((a + b) | 2) & c
}
test "low bits of the cut literal" {
    let dut = Cut { };
    dut.a = 0;
    dut.b = 0;
    dut.c = 1;
    step(1);
    assert_eq(dut.o3, 1);
    assert_eq(dut.o2, 0);
    dut.a = 1;
    step(1);
    assert_eq(dut.o3, 1);
    assert_eq(dut.o2, 1);
    dut.b = 1;
    step(1);
    assert_eq(dut.o3, 1);
    assert_eq(dut.o2, 0);
    dut.c = 0;
    step(1);
    assert_eq(dut.o3, 0);
    assert_eq(dut.o2, 0);
}
"#;
    sim("cut", src, 1);
}

/// 128 bitten geniş işaretli bağlamda -2^127: `-(a * b)` (a, b : i64) i129
/// tipindedir (§3.3: i64 · i64 → i128, `-` bir bit ekler), literal ona
/// sığar. |a · b| <= 2^126 olduğundan `-(a * b) > -2^127` her girdide
/// doğru, `-(a * b) < -2^127 + 1` her girdide yanlıştır (elle). En küçük
/// negatif değer örüntüsü (#116) yalnız W <= 128'de gerekir; 129 bitte
/// örüntü kuralı uygulansaydı literal +2^127 yazılırdı.
#[test]
fn issue_207_minus_two_pow_127_in_a_129_bit_signed_context() {
    let src = r#"module Neg129 {
    in  a  : i64
    in  b  : i64
    out gt : bool
    out lt : bool
    gt = -(a * b) > -170141183460469231731687303715884105728
    lt = -(a * b) < -170141183460469231731687303715884105727
}
test "-(a * b) stays above -2^127" {
    let dut = Neg129 { };
    dut.a = 0x8000_0000_0000_0000;
    dut.b = 0x8000_0000_0000_0000;
    step(1);
    assert_eq(dut.gt, true);
    assert_eq(dut.lt, false);
    dut.b = 0x7FFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.gt, true);
    assert_eq(dut.lt, false);
    dut.a = 0;
    step(1);
    assert_eq(dut.gt, true);
    assert_eq(dut.lt, false);
}
"#;
    sim("neg129", src, 1);
}

/// ADR-0115: parantezli negatif literalin dönüşümü (`(-K) as T`) çıplak
/// literal gibi hedef genişliğinde yazılır; sığmayan W2010 ile alt bitlere
/// kesilir. Değerler elle (8 bitlik örüntü): -128 → 0x80, -129 → 0x7F,
/// -1 → 0xFF. Önceden üretici E2005 veriyordu ("cannot determine the
/// width of the literal '-128'"), sığan `(-128) as i8` için de. Test dili
/// işaretsizdir: işaretli çıkış `as u8` ile okunur.
#[test]
fn issue_207_negated_literal_casts_have_their_value() {
    let src = r#"module NegCast {
    out a : u8
    out b : u8
    out c : u8
    out d : bits<8>
    let m128 : i8 = (-128) as i8
    let m129 : i8 = (-129) as i8
    a = m128 as u8
    b = m129 as u8
    c = (-1) as u8
    d = (-1) as bits<8>
}
test "negated literal casts" {
    let dut = NegCast { };
    step(1);
    assert_eq(dut.a, 0x80);
    assert_eq(dut.b, 0x7F);
    assert_eq(dut.c, 0xFF);
    assert_eq(dut.d, 0xFF);
}
"#;
    sim("negcast", src, 1);
}

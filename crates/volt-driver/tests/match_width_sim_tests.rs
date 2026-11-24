//! Match ifadesi `if` ifadesiyle aynı genişlikte üretilir (#215),
//! davranış olarak, gerçek Verilator'da.
//!
//! Şartname: type-inference.md §3.9 (ADR-0083) "match ifadesi çok kollu
//! `if` ifadesidir (§3.7), genişlik davranışı ayrışmaz"; sentez kipinde
//! somut kolların esnek aralıkları kesişir, literal kollar somut kola
//! uyarlanır, hepsi literalse `if` gibi `i32` (W2012). Sınanan sentezlenir
//! ve desenler onun doğal genişliğine denetlenir (ADR-0025). Her tasarım
//! aynı değeri bir `match` ve bir `if` biçimiyle hesaplar; beklenen
//! değerler yorumlarda elle hesaplandı, derleyici çıktısından alınmadı.
//! Kök konumdaki (`let m = match …`) biçim `tests/real_bugs/215_*`'tedir;
//! burada öteki konumlar: karşılaştırma işleneni, öz-belirlenimli indeks,
//! literal kollar, deyim `match`'inin sınananı ve Trit kolları.
//! Üretilen SV'nin metni karşılaştırılmaz; `volt test` Verilator'ın WIDTH
//! uyarılarında da durur, geçen test lint temizdir.
//!
//! Verilator yoksa simülasyon testleri atlanır; `VOLT_REQUIRE_TOOLS=
//! verilator` ile (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-match-width-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn volt(dir: &PathBuf, args: &[&str]) -> (bool, String) {
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
    (out.status.success(), text)
}

/// `src`'yi (tasarım + test blokları) `volt test` ile koşar; `passed` test
/// geçmeli, hiçbiri düşmemeli.
fn sim(tag: &str, src: &str, passed: usize) {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir(tag);
    std::fs::write(dir.join("case.volt"), src).expect("yaz");
    let (ok, text) = volt(&dir, &["test", "case.volt", "--target-dir", "build"]);
    // CI günlüğünde testin Verilator'da koştuğu görünsün (atlanan test de
    // "ok" yazar).
    eprintln!("--- volt test (match_width_sim_tests::{tag})\n{text}");
    assert!(ok, "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Karşılaştırmanın işleneni: `(match …) == 44` doğal genişlikte, 9 bitte
/// yapılır (ADR-0106). a = 200, b = 100, c = true: kol a + b = 300 ≠ 44,
/// iki biçim de `false`. Düzeltmeden önce match biçimi 8 bitte
/// karşılaştırıyordu: 300 mod 256 = 44, `true`.
#[test]
fn match_operand_of_a_comparison_keeps_its_carry() {
    sim(
        "compare",
        r#"module C {
    in  a  : u8
    in  b  : u8
    in  c  : bool
    out em : bool
    out ei : bool
    em = (match c { true => a + b, _ => a - b }) == 44
    ei = (if c { a + b } else { a - b }) == 44
}

test "the comparison sees the carry" {
    let dut = C { };
    dut.a = 200;
    dut.b = 100;
    dut.c = true;
    step(1);
    assert_eq(dut.em, false);
    assert_eq(dut.ei, false);
    // a = 22, b = 22: a + b = 44, both forms true.
    dut.a = 22;
    dut.b = 22;
    step(1);
    assert_eq(dut.em, true);
    assert_eq(dut.ei, true);
}
"#,
        1,
    );
}

/// Öz-belirlenimli konum (parça seçiminin başlangıcı): değer doğal
/// genişliğindedir (#117). a, b : u4, a = 12, b = 8, c = true: indeks
/// a + b = 20 (5 bit); p = 2^20 = 1048576 olduğundan p[20 +: 1] = 1.
/// Düzeltmeden önce match biçimi indeksi 4 bitte hesaplıyordu: 20 mod 16
/// = 4, p[4] = 0.
#[test]
fn match_index_keeps_its_carry() {
    sim(
        "index",
        r#"module X {
    in  p  : bits<32>
    in  a  : u4
    in  b  : u4
    in  c  : bool
    out sm : bits<1>
    out si : bits<1>
    sm = p[(match c { true => a + b, _ => a - b }) +: 1]
    si = p[(if c { a + b } else { a - b }) +: 1]
}

test "the index sees the carry" {
    let dut = X { };
    dut.p = 1048576;
    dut.a = 12;
    dut.b = 8;
    dut.c = true;
    step(1);
    assert_eq(dut.sm, 1);
    assert_eq(dut.si, 1);
    // c = false: a - b = 4, p[4] = 0.
    dut.c = false;
    step(1);
    assert_eq(dut.sm, 0);
    assert_eq(dut.si, 0);
}
"#,
        1,
    );
}

/// Kolları literal olan tipsiz `let` `i32`'dir (type-inference.md §3.9:
/// "hepsi tipsiz literalse `if` gibi", W2012): c = true → 5, c = false →
/// 6. Düzeltmeden önce match biçimi derlenmiyordu (E2005, aşağıdaki
/// `literal_arms_of_a_match_let_compile_like_if`).
#[test]
fn match_with_literal_arms_is_an_i32_like_if() {
    sim(
        "literal",
        r#"module L {
    in  c  : bool
    out ym : i32
    out yi : i32
    let lm = match c { true => 5, _ => 6 }
    let li = if c { 5 } else { 6 }
    ym = lm
    yi = li
}

test "literal arms" {
    let dut = L { };
    dut.c = true;
    step(1);
    assert_eq(dut.ym, 5);
    assert_eq(dut.yi, 5);
    dut.c = false;
    step(1);
    assert_eq(dut.ym, 6);
    assert_eq(dut.yi, 6);
}
"#,
        1,
    );
}

/// Deyim `match`'inin sınananı (ADR-0032) da doğal genişliktedir: a = 200,
/// b = 100 için a + b = 300, kol `300` seçilir (r = 2); `if` biçimi 9
/// bitte karşılaştırır (ri = 2). Düzeltmeden önce `case` 8 bitteydi ve
/// `300` `44`'ün etiketiyle aynıydı (Verilator CASEOVERLAP).
#[test]
fn statement_match_scrutinee_keeps_its_carry() {
    sim(
        "statement",
        r#"module S {
    in  clk : clock
    in  a   : u8
    in  b   : u8
    out rm  : u2
    out ri  : u2
    reg r : u2 = 0
    reg q : u2 = 0
    on clk {
        match a + b {
            44 => { r <= 1 }
            300 => { r <= 2 }
            _ => { r <= 3 }
        }
        if a + b == 44 { q <= 1 } else { if a + b == 300 { q <= 2 } else { q <= 3 } }
    }
    rm = r
    ri = q
}

test "the case sees the carry" {
    let dut = S { };
    dut.a = 200;
    dut.b = 100;
    step(1);
    assert_eq(dut.rm, 2);
    assert_eq(dut.ri, 2);
    // a + b = 44: the arm `44`.
    dut.a = 22;
    dut.b = 22;
    step(1);
    assert_eq(dut.rm, 1);
    assert_eq(dut.ri, 1);
    // a + b = 1: the `_` arm.
    dut.a = 1;
    dut.b = 0;
    step(1);
    assert_eq(dut.rm, 3);
    assert_eq(dut.ri, 3);
}
"#,
        1,
    );
}

/// Kolları Trit olan match ifadesi Trit'tir ve `Trit * x` çarpansız
/// eşlemeye iner (ADR-0062), `if` gibi: t = -1, x = 5 → -5 (i8'de 0xFB =
/// 251), t = +1 → 5, t = 0 → 0. Test dili negatif literal almaz; i8
/// değer ikiye tümleyen deseniyle okunur.
#[test]
fn match_of_trits_multiplies_like_if() {
    sim(
        "trit",
        r#"module T {
    in  c  : bool
    in  d  : bool
    in  x  : i8
    out ym : i8
    out yi : i8
    let neg : Trit = -1
    let pos : Trit = 1
    let zero : Trit = 0
    let t1 : Trit = if d { neg } else { pos }
    ym = (match c { true => t1, _ => zero }) * x
    yi = (if c { t1 } else { zero }) * x
}

test "trit arms" {
    let dut = T { };
    dut.x = 5;
    dut.c = true;
    dut.d = true;
    step(1);
    assert_eq(dut.ym, 251);
    assert_eq(dut.yi, 251);
    dut.d = false;
    step(1);
    assert_eq(dut.ym, 5);
    assert_eq(dut.yi, 5);
    dut.c = false;
    step(1);
    assert_eq(dut.ym, 0);
    assert_eq(dut.yi, 0);
}
"#,
        1,
    );
}

/// `Trit * x` (x i8) Trit değil, `x`'in tipidir (ADR-0062, type-inference.md
/// §3.3 "Trit × sayı → sayının tipi"); yalnız `Trit * Trit` Trit'tir. Çarpım
/// bir `let`'te ya da başka bir çarpımın işleneninde i8 değeriyle okunur:
/// t = +1, x = 5, z = 3 → (t * x) * z = 15; t = -1 → -15 (i16'da 65521).
/// Mutasyon kapısının kaçırdığı `trit.rs` `is_trit`'teki `&&` → `||`
/// mutantını öldürür (#215'in PR'ı): o mutantla `t * x` Trit sayılır ve
/// `p * z` Trit seçicisine iner (`p == 1 ? z : …`, 0).
#[test]
fn a_trit_times_a_number_is_the_number() {
    sim(
        "trit-product",
        r#"module P {
    in  d  : bool
    in  x  : i8
    in  z  : i8
    out y1 : i16
    out y2 : i16
    let neg : Trit = -1
    let pos : Trit = 1
    let t1 : Trit = if d { neg } else { pos }
    let p = t1 * x
    y1 = p * z
    y2 = (t1 * x) * z
}

test "a trit product is an i8, not a trit" {
    let dut = P { };
    dut.x = 5;
    dut.z = 3;
    dut.d = false;
    step(1);
    assert_eq(dut.y1, 15);
    assert_eq(dut.y2, 15);
    dut.d = true;
    step(1);
    assert_eq(dut.y1, 65521);
    assert_eq(dut.y2, 65521);
}
"#,
        1,
    );
}

/// Araçsız: kolları literal olan match `let`'i `if` biçimi gibi derlenir;
/// yalnız W2012 (iki `let` için iki kez) çıkar, hata yok. Düzeltmeden
/// önce üretici match biçiminde her literal için E2005 ("cannot determine
/// the width of the literal") veriyordu, tip denetimi `i32` dediği halde.
#[test]
fn literal_arms_of_a_match_let_compile_like_if() {
    let dir = temp_dir("literal-check");
    std::fs::write(
        dir.join("case.volt"),
        r#"module L {
    in  c  : bool
    out ym : i32
    out yi : i32
    let lm = match c { true => 5, _ => 6 }
    let li = if c { 5 } else { 6 }
    ym = lm
    yi = li
}
"#,
    )
    .expect("yaz");
    let (ok, text) = volt(&dir, &["build", "case.volt", "--target-dir", "build"]);
    assert!(ok, "{text}");
    assert!(!text.contains("E2005"), "{text}");
    assert_eq!(text.matches("warning[W2012]").count(), 2, "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

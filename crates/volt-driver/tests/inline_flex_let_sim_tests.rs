//! Açılmış fn gövdesindeki tipsiz `let`, çağrı `comb` içinde (ikame kipi)
//! ve modül düzeyinde (tel kipi) aynı değeri verir (#132), davranış
//! olarak, gerçek Verilator'da.
//!
//! Şartname: type-inference.md §3.3 ve ADR-0025: tipsiz `let`'in tipi
//! esnektir (`a + b`, a, b : u8 → `u8..u9`, değeri doğal genişliğinde
//! taşır); kullanıldığı işlem iki yanın aralıklarının kesişiminde yapılır
//! (`s | c`, c : u8 → 8 bit, sonuç u8). Bit işleminde kesişimin üst ucu
//! ile sonucun doğal genişliği aynıdır (8), ADR-0108'in A1 okumasının iki
//! seçeneği aynı değeri verir. ADR-0081 Karar 12.2 ve "Uygulamada
//! netleşenler": ikame kipi tel kipindeki genişliği korur.
//!
//! Her tasarım aynı fn'i `comb` içinde (`x`) ve modül düzeyinde (`y`)
//! çağırır; iki çıkış elle hesaplanan değeri vermeli. Düzeltmeden önce
//! ikame kipi `let`'in değerini doğal genişliğinde (`9'(9'(a) + 9'(b))`)
//! yerine yazıyor, kullanıldığı bit işlemi 8 bite daraltmıyordu: `x`
//! yanlış, ya da Verilator'ın WIDTH uyarısı (`volt test`'te ölümcül).
//! Üretilen SV'nin metni karşılaştırılmaz; geçen test lint temizdir.
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-fnlet-{tag}-{}", std::process::id()));
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
    eprintln!("--- volt test (inline_flex_let_sim_tests::{tag})\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// #132'nin örneği. `s = a + b` (u8..u9), `s | c` 8 bitte, `+ c` u9.
/// a = b = c = 255: s = 510, 8 bitte 254; 254 | 255 = 255; 255 + 255 =
/// 510. a = b = c = 128: s = 256, 8 bitte 0; 0 | 128 = 128; + 128 = 256.
/// Düzeltmeden önce `x`: 510 | 255 = 511, + 255 = 766 → 9 bitte 254; 256
/// | 128 = 384, + 128 = 512 → 0.
#[test]
fn issue_132_bitwise_operand_then_sum() {
    sim(
        "or-sum",
        r#"fn g(a: u8, b: u8, c: u8) -> u9 {
    let s = a + b;
    (s | c) + c
}
module M {
    in  a : u8
    in  b : u8
    in  c : u8
    out x : u9
    out y : u9
    comb { x = g(a, b, c) }
    y = g(a, b, c)
}
test "all ones" {
    let dut = M { };
    dut.a = 255;
    dut.b = 255;
    dut.c = 255;
    step(1);
    assert_eq(dut.x, 510);
    assert_eq(dut.y, 510);
}
test "carry only" {
    let dut = M { };
    dut.a = 128;
    dut.b = 128;
    dut.c = 128;
    step(1);
    assert_eq(dut.x, 256);
    assert_eq(dut.y, 256);
}
"#,
        2,
    );
}

/// Bit işleminin sonucu daha geniş tipe dönüştürülür (#132'nin bulduğu
/// WIDTHTRUNC sınıfı, `(s | p) as bits<46>`). `s | c` u8, `as u16` sıfırla
/// genişletir (ADR-0036). a = b = c = 128: s 8 bitte 0, 0 | 128 = 128.
/// Düzeltmeden önce `x`: 256 | 128 = 384.
#[test]
fn issue_132_bitwise_operand_under_a_widening_cast() {
    sim(
        "or-cast",
        r#"fn h(a: u8, b: u8, c: u8) -> u16 {
    let s = a + b;
    (s | c) as u16
}
module M {
    in  a : u8
    in  b : u8
    in  c : u8
    out x : u16
    out y : u16
    comb { x = h(a, b, c) }
    y = h(a, b, c)
}
test "carry dropped" {
    let dut = M { };
    dut.a = 128;
    dut.b = 128;
    dut.c = 128;
    step(1);
    assert_eq(dut.x, 128);
    assert_eq(dut.y, 128);
}
"#,
        1,
    );
}

/// `~` işlenenli bit işlemi (#132'nin bulduğu WIDTHEXPAND sınıfı, `s ^
/// ~p as u14`). `~c` u8 (§3.4), `s ^ ~c` 8 bitte. a = b = 128, c = 0: s 8
/// bitte 0, ~0 = 255, 0 ^ 255 = 255; `as u14` 255. Düzeltmeden önce `x`:
/// 256 ^ 255 = 511.
#[test]
fn issue_132_xor_with_a_complement_under_a_cast() {
    sim(
        "xor-not",
        r#"fn k(a: u8, b: u8, c: u8) -> u14 {
    let s = a + b;
    (s ^ ~c) as u14
}
module M {
    in  a : u8
    in  b : u8
    in  c : u8
    out x : u14
    out y : u14
    comb { x = k(a, b, c) }
    y = k(a, b, c)
}
test "carry dropped" {
    let dut = M { };
    dut.a = 128;
    dut.b = 128;
    dut.c = 0;
    step(1);
    assert_eq(dut.x, 255);
    assert_eq(dut.y, 255);
}
"#,
        1,
    );
}

/// Denetim: `let`'in değeri kendi doğal genişliğinde okunduğunda taşma
/// biti kalır (u9 dönüşte `s`, ADR-0025). a = b = 255: 510. Düzeltmeden
/// önce de geçiyordu; daraltma yalnız dar işlemde olmalı.
#[test]
fn issue_132_control_full_width_use_keeps_the_carry() {
    sim(
        "full",
        r#"fn f(a: u8, b: u8) -> u9 {
    let s = a + b;
    s
}
module M {
    in  a : u8
    in  b : u8
    out x : u9
    out y : u9
    comb { x = f(a, b) }
    y = f(a, b)
}
test "carry kept" {
    let dut = M { };
    dut.a = 255;
    dut.b = 255;
    step(1);
    assert_eq(dut.x, 510);
    assert_eq(dut.y, 510);
}
"#,
        1,
    );
}

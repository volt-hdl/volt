//! Trit aritmetiğinin genişliği tip denetiminden (#463, ADR-0145): SV
//! üreticisi `-t`'nin, Trit'li çarpımın ve `t ± u`'nun genişliğini
//! hesaplamaz, tip denetiminin tablosundan okur. Bu test #463'ün sonuçlarını
//! ve tablonun ulaşması gereken yolları gerçek Verilator'da sınar:
//!
//! - `e = t * x == 1`: literal karşı yanın tipinde (i8) boyutlanır; önce
//!   E2005 alıyordu;
//! - `let n = -t`: Trit (2 bit); önce 3 bitlik teldi, `n * t`'nin seçicisi
//!   Verilator'da WIDTH uyarısı verdi, `volt test` durdu;
//! - `comb` yereli `let s = -(t * x)`: i9; önce 8 bitte alınıyordu;
//! - fn açılımı (ADR-0081): tel kipinde gövdenin `let`'i (`scale`), ikame
//!   kipinde gövdenin `let` şablonunun kopyası (`negscale`), argümanı Trit
//!   çarpımı olan çağrı (`neg(t * x)`) ve modül `let`'inin bütün sağ tarafı
//!   olan çağrı (`mac`). Kopya düğümler tabloda yoktur; değerlerini
//!   gövdedeki özgün düğümden okurlar (`InlineNotes::origins`). Okuyamazlarsa
//!   E2005 alırlar.
//!
//! Değerler spec'ten elle hesaplandı. `Trit * iN` iN'dir, ternary MAC'tir:
//! +1 → x, -1 → -x, 0 → 0 (type-inference.md §3.3); `-z` (z : iN) i(N+1)
//! (§3.4); `-Trit` Trit, -(+1) = -1 (§3.4, ADR-0003). Test dili portları bit
//! deseniyle yazar ve okur: i8'de -128 = 128, -5 = 251; i9'da -1 = 511;
//! Trit'te ve i2'de -1 = 3.
//!
//! Verilator yoksa test atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile (CI'ın
//! araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-tritwidth-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Çalıştırılan `volt` komutunun çıktısı: (başarılı mı, stdout + stderr).
fn volt(tag: &str, src: &str, args: &[&str]) -> (bool, String) {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("trit_width_test.volt"), src).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(args)
        .output()
        .expect("volt");
    let _ = std::fs::remove_dir_all(&dir);
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text)
}

const DESIGN: &str = "\
fn scale(t : Trit, x : i8) -> i8 {
    let s = t * x
    s
}

fn negscale(t : Trit, x : i8) -> i9 {
    let s = t * x
    let m = -s
    m
}

fn neg(z : i8) -> i9 {
    -z
}

fn mac(t : Trit, x : i8) -> i8 {
    t * x
}

module TritWidths {
    in  tp : bool
    in  tn : bool
    in  x  : i8
    out a  : i8
    out b  : i9
    out c  : i9
    out d  : i8
    out e  : bool
    out f  : i9
    out g  : i2
    out h  : Trit

    let t : Trit = if tp { 1 } else if tn { -1 } else { 0 }
    let n = -t
    let r = mac(t, x)

    a = scale(t, x)
    d = r
    e = t * x == 1
    g = n as i2
    h = n * t
    comb {
        let s = -(t * x)
        f = s
        b = negscale(t, x)
        c = neg(t * x)
    }
}

test \"plus one and the smallest i8\" {
    let dut = TritWidths { };
    dut.tp = true;
    dut.tn = false;
    dut.x = 128;
    step(1);
    // t = +1, x = -128: t * x = -128 (pattern 128); -(t * x) = 128 in i9.
    // An 8-bit minus gives -128 again: i9 pattern 384.
    assert_eq(dut.a, 128);
    assert_eq(dut.b, 128);
    assert_eq(dut.c, 128);
    assert_eq(dut.d, 128);
    assert_eq(dut.e, false);
    assert_eq(dut.f, 128);
    // n = -t = -1: i2 and Trit pattern 3; n * t = -1 * +1 = -1.
    assert_eq(dut.g, 3);
    assert_eq(dut.h, 3);
}

test \"minus one and five\" {
    let dut = TritWidths { };
    dut.tp = false;
    dut.tn = true;
    dut.x = 5;
    step(1);
    // t = -1: t * x = -5 (pattern 251); -(t * x) = 5.
    assert_eq(dut.a, 251);
    assert_eq(dut.b, 5);
    assert_eq(dut.c, 5);
    assert_eq(dut.d, 251);
    assert_eq(dut.e, false);
    assert_eq(dut.f, 5);
    // n = -(-1) = +1; n * t = +1 * -1 = -1.
    assert_eq(dut.g, 1);
    assert_eq(dut.h, 3);
}

test \"plus one times one equals the literal\" {
    let dut = TritWidths { };
    dut.tp = true;
    dut.tn = false;
    dut.x = 1;
    step(1);
    // t * x = 1 == 1; -(t * x) = -1 in i9: pattern 511.
    assert_eq(dut.e, true);
    assert_eq(dut.f, 511);
}

test \"zero\" {
    let dut = TritWidths { };
    dut.tp = false;
    dut.tn = false;
    dut.x = 5;
    step(1);
    // t = 0: every product, its minus and -t are 0.
    assert_eq(dut.a, 0);
    assert_eq(dut.b, 0);
    assert_eq(dut.c, 0);
    assert_eq(dut.d, 0);
    assert_eq(dut.e, false);
    assert_eq(dut.f, 0);
    assert_eq(dut.g, 0);
    assert_eq(dut.h, 0);
}
";

/// Main'de (c4f24632) tasarım E2005 ile derlenmedi; `e` satırı olmadan
/// Verilator `n`'nin 3 bitlik teli yüzünden WIDTHEXPAND/WIDTHTRUNC verdi.
#[test]
fn trit_widths_come_from_the_type_checker_in_verilator() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let (_, out) = volt(
        "sim",
        DESIGN,
        &["test", "trit_width_test.volt", "--target-dir", "build"],
    );
    assert!(out.contains("test result: ok. 4 passed; 0 failed"), "{out}");
}

/// Araç gerektirmez: aynı tasarım `volt check`'ten ve `volt build`'den
/// tanısız geçer (E2005 yok; fn açılımının kopyaları tabloyu okur).
#[test]
fn the_design_builds_without_a_diagnostic() {
    let (ok, out) = volt("check", DESIGN, &["check", "trit_width_test.volt"]);
    assert!(ok, "{out}");
    assert!(out.contains("Result 0 error(s), 0 warning(s)"), "{out}");
    let (ok, out) = volt("build", DESIGN, &["build", "trit_width_test.volt"]);
    assert!(ok, "{out}");
    assert!(!out.contains("error") && !out.contains("warning"), "{out}");
}

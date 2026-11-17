//! İşaretli `>>`, genişlik kadar ya da daha fazla sabit miktarla (#302),
//! davranış olarak, gerçek Verilator'da (`volt test`).
//!
//! Kural (ADR-0120, ADR-0036 §1, const-eval.md'nin i128 `>>`'i, IEEE
//! 1800-2017 §11.4.10): işaretli `>>` aritmetiktir; miktar K genişliğe
//! (w) eşit ya da büyükse bütün bitler boşalır ve işaret bitiyle dolar.
//! Negatif değerin sonucu bütün bitleri 1 olan değerdir (-1), negatif
//! olmayanınki 0. Beklenen değerler bu kuraldan elle yazıldı (her testin
//! yorumunda), derleyici ya da simülatör çıktısından alınmadı. Üretilen
//! SV'nin metni karşılaştırılmaz.
//!
//! Düzeltmeden önce üretici `a >>> K`'yi olduğu gibi yazıyordu; Verilator
//! 5.050–5.052 sabit K, C++ kelime genişliğine (cppw: w ≤ 32 için 32,
//! 33–64 için 64, üstünde 32·⌈w/32⌉) eşit ya da büyük ve w cppw'nin katı
//! değilken negatif değer için 0 verir (#302'deki tarama). Her test bu
//! taramanın bir genişlik sınıfıdır: cppw 32 (dar ve geniş), cppw 64,
//! 64 bitten geniş bileşik değer (tek ve çift genişlik; i65, 5.054'te de
//! kalan dar hatanın sınıfıdır, w mod 32 = 1). Düzeltmeyle üretici
//! `a >>> (w-1)` yazar (ADR-0120): IEEE'ye göre aynı değer, w-1 < cppw.
//!
//! Test dili 64 bitlik değer taşır ve negatif literal almaz: işaretli
//! girişler ikiye tümleyen örüntüsüyle yazılır (i63'te -1 =
//! 0x7FFF_FFFF_FFFF_FFFF); 64 bitten geniş sonuç `bits<64>` dilimleriyle
//! okunur.
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-overshift-{tag}-{}", std::process::id()));
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
    eprintln!("--- volt test (signed_overshift_sim_tests::{tag})\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// #302'nin örneği: `i63 >> 64` (cppw 64, w = 63). -1, en küçük değer ve
/// 0x5555… negatif: 63 birli örüntü; en büyük değer ve 1 negatif değil: 0.
#[test]
fn issue_302_i63_shifted_by_64_fills_with_the_sign() {
    sim(
        "i63",
        r#"module M {
    in  a : i63
    out y : i63
    y = a >> 64
}
test "negative values give all ones" {
    let dut = M { };
    dut.a = 0x7FFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.y, 0x7FFF_FFFF_FFFF_FFFF);
    dut.a = 0x4000_0000_0000_0000;
    step(1);
    assert_eq(dut.y, 0x7FFF_FFFF_FFFF_FFFF);
    dut.a = 0x5555_5555_5555_5555;
    step(1);
    assert_eq(dut.y, 0x7FFF_FFFF_FFFF_FFFF);
}
test "non-negative values give 0" {
    let dut = M { };
    dut.a = 0x3FFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.y, 0);
    dut.a = 1;
    step(1);
    assert_eq(dut.y, 0);
}
"#,
        2,
    );
}

/// cppw 32: `i31 >> 32` (K = w + 1 = cppw). i31'de -1 = 0x7FFF_FFFF, en
/// küçük değer 0x4000_0000: 31 birli örüntü; 0x3FFF_FFFF: 0.
#[test]
fn an_i31_shifted_by_32_fills_with_the_sign() {
    sim(
        "i31",
        r#"module M {
    in  a : i31
    out y : i31
    y = a >> 32
}
test "negative values give all ones" {
    let dut = M { };
    dut.a = 0x7FFF_FFFF;
    step(1);
    assert_eq(dut.y, 0x7FFF_FFFF);
    dut.a = 0x4000_0000;
    step(1);
    assert_eq(dut.y, 0x7FFF_FFFF);
}
test "non-negative values give 0" {
    let dut = M { };
    dut.a = 0x3FFF_FFFF;
    step(1);
    assert_eq(dut.y, 0);
}
"#,
        2,
    );
}

/// cppw 32, dar değer: `i8 >> 32` (K = cppw; #302'deki taramada w = 1–15
/// yalnız SV taramasında görüldü). i8'de -128 = 0x80, -1 = 0xFF: 8 birli
/// örüntü 0xFF; 0x7F: 0.
#[test]
fn an_i8_shifted_by_32_fills_with_the_sign() {
    sim(
        "i8",
        r#"module M {
    in  a : i8
    out y : i8
    y = a >> 32
}
test "negative values give all ones" {
    let dut = M { };
    dut.a = 0x80;
    step(1);
    assert_eq(dut.y, 0xFF);
    dut.a = 0xFF;
    step(1);
    assert_eq(dut.y, 0xFF);
}
test "non-negative values give 0" {
    let dut = M { };
    dut.a = 0x7F;
    step(1);
    assert_eq(dut.y, 0);
}
"#,
        2,
    );
}

/// 64 bitten geniş bileşik değer, w mod 32 = 1: `(e + f) >> 130`,
/// `e, f : i64`, toplam i65 (cppw 96, K = 2w). Verilator 5.054'te kalan
/// dar hatanın da örneği. e = -1, f = 0: toplam -1, 65 bitin hepsi 1;
/// e = f = -2^63: toplam -2^64, negatif. e = f = 2^63 - 1: toplam pozitif,
/// 0. Dilimler [64:1] ve [63:0].
#[test]
fn a_65_bit_signed_sum_shifted_by_130_fills_with_the_sign() {
    sim(
        "i65",
        r#"module M {
    in  e  : i64
    in  f  : i64
    out hi : bits<64>
    out lo : bits<64>
    hi = ((e + f) >> 130)[64:1]
    lo = ((e + f) >> 130)[63:0]
}
test "negative sums give all ones" {
    let dut = M { };
    dut.e = 0xFFFF_FFFF_FFFF_FFFF;
    dut.f = 0;
    step(1);
    assert_eq(dut.hi, 0xFFFF_FFFF_FFFF_FFFF);
    assert_eq(dut.lo, 0xFFFF_FFFF_FFFF_FFFF);
    dut.e = 0x8000_0000_0000_0000;
    dut.f = 0x8000_0000_0000_0000;
    step(1);
    assert_eq(dut.hi, 0xFFFF_FFFF_FFFF_FFFF);
    assert_eq(dut.lo, 0xFFFF_FFFF_FFFF_FFFF);
}
test "non-negative sums give 0" {
    let dut = M { };
    dut.e = 0x7FFF_FFFF_FFFF_FFFF;
    dut.f = 0x7FFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.hi, 0);
    assert_eq(dut.lo, 0);
}
"#,
        2,
    );
}

/// 64 bitten geniş bileşik değer, çift genişlik: `a * b` (`a, b : i33`,
/// i66, cppw 96, K = 2w). a = -1 (i33'te 0x1_FFFF_FFFF), b = 1: çarpım
/// -1, 66 bitin hepsi 1; a = 2, b = 3: 6, pozitif, 0. Dilimler [65:2] ve
/// [63:0].
#[test]
fn a_66_bit_signed_product_shifted_by_132_fills_with_the_sign() {
    sim(
        "i66",
        r#"module M {
    in  a  : i33
    in  b  : i33
    out hi : bits<64>
    out lo : bits<64>
    hi = ((a * b) >> 132)[65:2]
    lo = ((a * b) >> 132)[63:0]
}
test "negative products give all ones" {
    let dut = M { };
    dut.a = 0x1_FFFF_FFFF;
    dut.b = 1;
    step(1);
    assert_eq(dut.hi, 0xFFFF_FFFF_FFFF_FFFF);
    assert_eq(dut.lo, 0xFFFF_FFFF_FFFF_FFFF);
}
test "non-negative products give 0" {
    let dut = M { };
    dut.a = 2;
    dut.b = 3;
    step(1);
    assert_eq(dut.hi, 0);
    assert_eq(dut.lo, 0);
}
"#,
        2,
    );
}

/// 64 bitten geniş bileşik değer, tek genişlik, w mod 32 = 3:
/// `a * b + c * d` (`i33` girişler, i67, cppw 96, K = 2w). a = -1, b = 1,
/// c = d = 0: toplam -1, 67 bitin hepsi 1; a = 2, b = 3: 6, pozitif, 0.
/// Dilim [66:3].
#[test]
fn a_67_bit_signed_sum_of_products_shifted_by_134_fills_with_the_sign() {
    sim(
        "i67",
        r#"module M {
    in  a  : i33
    in  b  : i33
    in  c  : i33
    in  d  : i33
    out hi : bits<64>
    hi = ((a * b + c * d) >> 134)[66:3]
}
test "negative sums give all ones" {
    let dut = M { };
    dut.a = 0x1_FFFF_FFFF;
    dut.b = 1;
    dut.c = 0;
    dut.d = 0;
    step(1);
    assert_eq(dut.hi, 0xFFFF_FFFF_FFFF_FFFF);
}
test "non-negative sums give 0" {
    let dut = M { };
    dut.a = 2;
    dut.b = 3;
    dut.c = 0;
    dut.d = 0;
    step(1);
    assert_eq(dut.hi, 0);
}
"#,
        2,
    );
}

/// `w` bitlik maske (w ≤ 64).
fn mask(w: u32) -> u64 {
    if w >= 64 {
        u64::MAX
    } else {
        (1u64 << w) - 1
    }
}

/// Verilator'ın C++ kelime genişliği (#302'deki tarama): w ≤ 32 için 32,
/// 33–64 için 64, üstünde 32·⌈w/32⌉.
fn cppw(w: u32) -> u32 {
    match w {
        0..=32 => 32,
        33..=64 => 64,
        _ => w.div_ceil(32) * 32,
    }
}

/// Denenen sabit miktarlar: w - 1 (W2013 yok, değişmeyen yol), w, w + 1,
/// cppw, cppw + 1, 2w, 2^31 - 1, 2^32 - 1 (tekrarsız, artan).
fn amounts(w: u32) -> Vec<u64> {
    let w64 = u64::from(w);
    let c = u64::from(cppw(w));
    let mut k = vec![
        w64 - 1,
        w64,
        w64 + 1,
        c,
        c + 1,
        2 * w64,
        (1 << 31) - 1,
        (1 << 32) - 1,
    ];
    k.sort_unstable();
    k.dedup();
    k
}

/// Her genişlik sınıfında, her denenen miktarla üretilen SV'nin değeri
/// (ADR-0120'nin `>>> (w-1)` biçimi). Beklenen değer kuraldan: K ≥ w - 1
/// için sonuç işaret bitiyle dolar, negatif değerde bütün bitler 1, değilse
/// 0. Sabitlenen Verilator (5.052) K ≥ cppw'de negatif değere 0 verir
/// (kanarya, `verilator_canary_tests.rs`): test, üreticinin bütün
/// sınıflarda bu miktarları yazmadığını metne bakmadan gösterir.
///
/// Portlar i1–i64 (girişler -1, en küçük, en büyük, 0x5555…); bileşik
/// değerler i65 (`e + f`), i66 (`c * d`), i67 (`c * d + c * d`), i96
/// (`g * h`, cppw'nin katı, hatanın dışında), i97, i127 (`r * s + r * s`), i128
/// (`p * q`, katı) ve i129 (`p * q + p * q`, w mod 32 = 1), [w-1:w-64] ve
/// [63:0] dilimleriyle. Aynı portlarda K ≥ w için işaretsiz `>>` ve
/// işaretli `<<` 0 verir: üretici yalnız işaretli `>>`'nin miktarını
/// değiştirir.
#[test]
fn every_width_class_fills_with_the_sign_for_every_constant_amount() {
    const WIDTHS: [u32; 13] = [1, 2, 7, 8, 15, 16, 17, 31, 32, 33, 48, 63, 64];
    let mut ports = String::new();
    let mut body = String::new();
    // (çıkış, port genişliği) — beklenen değer port değerinin işaretinden.
    let mut simple: Vec<(String, u32)> = Vec::new();
    // Değeri her zaman 0 olan çıkışlar: işaretsiz `>>` ve işaretli `<<`,
    // K ≥ w (ADR-0120 §1; üretici bunların miktarına dokunmaz).
    let mut zero: Vec<String> = Vec::new();
    for w in WIDTHS {
        ports.push_str(&format!("    in  a{w} : i{w}\n"));
        ports.push_str(&format!("    in  b{w} : u{w}\n"));
        for (i, k) in amounts(w).into_iter().enumerate() {
            let y = format!("y{w}_{i}");
            ports.push_str(&format!("    out {y} : i{w}\n"));
            body.push_str(&format!("    {y} = a{w} >> {k}\n"));
            simple.push((y, w));
            if k < u64::from(w) {
                continue;
            }
            let (u, l) = (format!("u{w}_{i}"), format!("l{w}_{i}"));
            ports.push_str(&format!("    out {u} : u{w}\n    out {l} : i{w}\n"));
            body.push_str(&format!("    {u} = b{w} >> {k}\n    {l} = a{w} << {k}\n"));
            zero.push(u);
            zero.push(l);
        }
    }
    // Bileşik değerler: (ad, ifade, genişlik). İşaret, vektör başına
    // aşağıda elle.
    let composites: [(&str, &str, u32); 8] = [
        ("s65", "e + f", 65),
        ("m66", "c * d", 66),
        ("m67", "c * d + c * d", 67),
        ("m96", "g * h", 96),
        ("m97", "g * h + g * h", 97),
        ("m127", "r * s + r * s", 127),
        ("m128", "p * q", 128),
        ("m129", "p * q + p * q", 129),
    ];
    for name in ["e", "f", "p", "q"] {
        ports.push_str(&format!("    in  {name} : i64\n"));
    }
    for name in ["r", "s"] {
        ports.push_str(&format!("    in  {name} : i63\n"));
    }
    for name in ["c", "d"] {
        ports.push_str(&format!("    in  {name} : i33\n"));
    }
    for name in ["g", "h"] {
        ports.push_str(&format!("    in  {name} : i48\n"));
    }
    // (çıkış, bileşiğin sırası)
    let mut wide: Vec<(String, usize)> = Vec::new();
    for (ci, (name, expr, w)) in composites.iter().enumerate() {
        for (i, k) in amounts(*w).into_iter().enumerate() {
            for (part, hi, lo) in [("h", w - 1, w - 64), ("l", 63, 0)] {
                let y = format!("{name}_{i}{part}");
                ports.push_str(&format!("    out {y} : bits<64>\n"));
                body.push_str(&format!("    {y} = (({expr}) >> {k})[{hi}:{lo}]\n"));
                wide.push((y, ci));
            }
        }
    }
    let mut src = format!("module M {{\n{ports}{body}}}\n");

    // Port vektörleri, ikiye tümleyen örüntüsü: -1, en küçük, en büyük,
    // 0x5555… (i1'de en küçük = -1 = 1, en büyük = 0).
    let port_vectors: [fn(u32) -> u64; 4] = [
        mask,
        |w| 1u64 << (w - 1),
        |w| mask(w) >> 1,
        |w| 0x5555_5555_5555_5555 & mask(w),
    ];
    // Bileşik vektörler: (e, f, c, d, g, h, p, q, r, s) örüntüleri ve her
    // bileşiğin işareti (true: negatif), elle:
    // 0: e = -1, f = 0 → -1; c = -1, d = 1 → -1, toplam -2; g = -1, h = 1
    //    → -1, -2; p = -1, q = 1 → -1, -2; r = -1, s = 1 → -1, -2. Hepsi
    //    negatif.
    // 1: e = 1, f = 0; c = 2, d = 3; g = 2, h = 3; p = 2, q = 3; r = s = 3:
    //    hepsi pozitif.
    // 2: e = f = -2^63 → -2^64; c = -2^32, d = 1 → -2^32, toplam -2^33;
    //    g = -2^47, h = 1; p = -2^63, q = 1; r = -2^62, s = 1. Hepsi negatif.
    // 3: e = f = 2^63 - 1 → pozitif; c = d = -2^32 → 2^64, pozitif;
    //    g = h = -2^47 → 2^94; p = q = -2^63 → 2^126, toplam 2^127,
    //    pozitif; r = s = -2^62 → 2^124, pozitif. Hepsi pozitif.
    let m = |w: u32| mask(w);
    let min = |w: u32| 1u64 << (w - 1);
    let wide_vectors: [([u64; 10], bool); 4] = [
        ([m(64), 0, m(33), 1, m(48), 1, m(64), 1, m(63), 1], true),
        ([1, 0, 2, 3, 2, 3, 2, 3, 3, 3], false),
        (
            [
                min(64),
                min(64),
                min(33),
                1,
                min(48),
                1,
                min(64),
                1,
                min(63),
                1,
            ],
            true,
        ),
        (
            [
                m(64) >> 1,
                m(64) >> 1,
                min(33),
                min(33),
                min(48),
                min(48),
                min(64),
                min(64),
                min(63),
                min(63),
            ],
            false,
        ),
    ];
    for (v, (pv, (wv, negative))) in port_vectors.iter().zip(wide_vectors).enumerate() {
        src.push_str(&format!("test \"vector {v}\" {{\n    let dut = M {{ }};\n"));
        for w in WIDTHS {
            src.push_str(&format!("    dut.a{w} = {:#x};\n", pv(w)));
            src.push_str(&format!("    dut.b{w} = {:#x};\n", pv(w)));
        }
        for (name, value) in ["e", "f", "c", "d", "g", "h", "p", "q", "r", "s"]
            .iter()
            .zip(wv)
        {
            src.push_str(&format!("    dut.{name} = {value:#x};\n"));
        }
        src.push_str("    step(1);\n");
        for (y, w) in &simple {
            let value = pv(*w);
            let negative = (value >> (w - 1)) & 1 == 1;
            let want = if negative { mask(*w) } else { 0 };
            src.push_str(&format!("    assert_eq(dut.{y}, {want:#x});\n"));
        }
        for (y, _) in &wide {
            let want = if negative { u64::MAX } else { 0 };
            src.push_str(&format!("    assert_eq(dut.{y}, {want:#x});\n"));
        }
        for y in &zero {
            src.push_str(&format!("    assert_eq(dut.{y}, 0);\n"));
        }
        src.push_str("}\n");
    }
    sim("sweep", &src, 4);
}

/// Üreticinin sabit saydığı miktar biçimleri (ADR-0120 §2): `const`, tipli
/// literal, sabit aritmetiği, işaretsiz tipe `as` (hedefin genişliğine
/// keser: `320 as u8` = 64, `300 as u8` = 44), satır içine açılan fonksiyon
/// argümanı ve generic parametre. i63'te 64 ve 65 genişliği aşar: sonuç
/// işaret bitiyle dolar. 44 aşmaz: `a >> 44` aritmetik kaydırmadır.
/// a = -2^62 (0x4000_0000_0000_0000): taşanlar 63 birli örüntü;
/// -2^62 >> 44 = -2^18 = 2^63 - 2^18 = 0x7FFF_FFFF_FFFC_0000.
/// a = 2^61: taşanlar 0; 2^61 >> 44 = 2^17 = 0x2_0000.
#[test]
fn every_constant_amount_form_fills_with_the_sign() {
    sim(
        "forms",
        r#"const K : u32 = 64
fn sh(x : i63, k : u32) -> i63 {
    x >> k
}
module G<const N : u32> {
    in  a : i63
    out y : i63
    y = a >> N
}
module M {
    in  a  : i63
    out yc : i63
    out yt : i63
    out yk : i63
    out ya : i63
    out yw : i63
    out yn : i63
    out yf : i63
    out yg : i63
    yc = a >> K
    yt = a >> 64u8
    yk = a >> (K + 1)
    ya = a >> (64 as u8)
    yw = a >> (320 as u8)
    yn = a >> (300 as u8)
    yf = sh(a, 64)
    let g = G<64> { a: a }
    yg = g.y
}
test "a negative value" {
    let dut = M { };
    dut.a = 0x4000_0000_0000_0000;
    step(1);
    assert_eq(dut.yc, 0x7FFF_FFFF_FFFF_FFFF);
    assert_eq(dut.yt, 0x7FFF_FFFF_FFFF_FFFF);
    assert_eq(dut.yk, 0x7FFF_FFFF_FFFF_FFFF);
    assert_eq(dut.ya, 0x7FFF_FFFF_FFFF_FFFF);
    assert_eq(dut.yw, 0x7FFF_FFFF_FFFF_FFFF);
    assert_eq(dut.yn, 0x7FFF_FFFF_FFFC_0000);
    assert_eq(dut.yf, 0x7FFF_FFFF_FFFF_FFFF);
    assert_eq(dut.yg, 0x7FFF_FFFF_FFFF_FFFF);
}
test "a positive value" {
    let dut = M { };
    dut.a = 0x2000_0000_0000_0000;
    step(1);
    assert_eq(dut.yc, 0);
    assert_eq(dut.yt, 0);
    assert_eq(dut.yk, 0);
    assert_eq(dut.ya, 0);
    assert_eq(dut.yw, 0);
    assert_eq(dut.yn, 0x2_0000);
    assert_eq(dut.yf, 0);
    assert_eq(dut.yg, 0);
}
"#,
        2,
    );
}

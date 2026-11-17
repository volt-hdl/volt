//! Aynı genişlikte ve aynı işaretli `as` zinciri, daha geniş bir bağlamda
//! (#303), davranış olarak, gerçek Verilator'da (`volt test`).
//!
//! Kural: `as` kaynağı doğal genişliğine sabitler (ADR-0025); `bits<N> ↔
//! uN` ve `uN → uN` aynı bitlerin yeniden yorumudur (type-inference.md
//! §3.6); daha geniş, aynı işaretli hedefe atama ya da dönüşüm sıfır
//! (işaretlide işaret) genişletmedir (type-inference.md §5, ADR-0041,
//! sv-mapping.md §6). Beklenen değerler bu kurallardan elle yazıldı (her
//! testin yorumunda); derleyici ya da simülatör çıktısından alınmadı.
//! Üretilen SV'nin metni karşılaştırılmaz.
//!
//! Düzeltmeden önce üretici böyle bir zincirin bileşik işlenenini sınırsız
//! basıyordu (`49'(((a | 43'h…) & 43'h…))`, `11'((s ^ 8'sh35 & t))`): SV
//! işlenenleri hedef genişlikte hesaplar, Verilator WIDTHEXPAND verir ve
//! uyarı varsayılan olarak ölümcül olduğundan `volt test` dururdu.
//! Testler #303'ün beş bulgusunu ve #303 taramasının her sınıfını kapsar:
//! iki ya da üç aynı genişlik dönüşümü (`as bits<n> as un`, `as un as un`,
//! `as bits<n> as un as un`) daha geniş hedefe, tipli `let`e, daha geniş
//! aritmetiğin işleneni olarak; zincir bir bit işleminin işleneni olarak;
//! işaretli zincirin genişletilmesi (`as i8 as i11`), daha geniş
//! karşılaştırma ve aritmetik içinde.
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-castchain-{tag}-{}", std::process::id()));
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
    eprintln!("--- volt test (cast_chain_sim_tests::{tag})\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// #303'ün başlığındaki örnek (run 44075586023, `o241w`): u43 ifade
/// `as bits<43> as u43`, u49 hedefe. İki dönüşüm aynı 43 biti yeniden
/// yorumlar; hedef sıfır genişletir, 43-48. bitler 0.
/// K1 = 0x3612E6A3554, K2 = 0x4CBBBD40467.
/// - a'nın 43 biti 1: a | K1 hepsi 1, & K2 = K2 = 0x4CBBBD40467.
/// - a = 0: K1 & K2, onaltılık basamak basamak (3&4, 6&C, 1&B, 2&B, E&B,
///   6&D, A&4, 3&0, 5&4, 5&6, 4&7) = 0,4,1,2,A,4,0,0,4,4,4 = 0x412A400444.
/// - a = 2^42 (yalnız en üst bit): a | K1 = 0x7612E6A3554; & K2'nin en
///   üst basamağı 7&4 = 4: 0x4412A400444. Bit 42 korunur, 43 ve üstü 0.
#[test]
fn issue_303_bits_cast_chain_into_a_wider_target() {
    sim(
        "o241w",
        r#"module M {
    in  a : u43
    out y : u49
    y = ((a | 0x3612E6A3554) & 0x4CBBBD40467) as bits<43> as u43
}
test "all ones keeps the second mask" {
    let dut = M { };
    dut.a = 0x7FF_FFFF_FFFF;
    step(1);
    assert_eq(dut.y, 0x4CBBBD40467);
}
test "zero gives the two masks anded" {
    let dut = M { };
    dut.a = 0;
    step(1);
    assert_eq(dut.y, 0x412A400444);
}
test "the top source bit stays, the widened bits are zero" {
    let dut = M { };
    dut.a = 0x400_0000_0000;
    step(1);
    assert_eq(dut.y, 0x4412A400444);
}
"#,
        3,
    );
}

/// #303'ün diğer dört bulgusu, issue'daki port ve hedef genişlikleriyle.
/// - `o191w` (u57): p252 / p252 = 1 (p252 ≠ 0); `p160[54:2] as u53` =
///   p160 >> 2'nin alt 53 biti; & 3. p160 = 4 → 1, & 1 & 3 = 1;
///   p160 = 0 → 0. Sıfıra bölme değeri bu testin konusu değil.
/// - `o168w` (u23): 289327 - 27184 = 262143 = 0x3FFFF (18 bit 1), & p223 =
///   p223. p235 negatif değil (31): `as u18` değeri korur (31). 31 ^
///   0x2AAAA: alt beş bit 01010 ^ 11111 = 10101 → 0x2AAB5.
/// - `o190w` (u3): p270 = 0x20000, bit 17 = 1 (i20'de pozitif) → 1 | … = 1;
///   p270 = 0, p179 = 1, p271 = 0 → 0 | 0 = 0; p271 = 1 → 1.
/// - `o244w` (u10): p151 = 0x7F0 → p151[10:4] = 0x7F; p29 = 2:
///   (2 | 65) = 67 = 0x43, & 0x7F = 0x43. p151 = 0x10 → [10:4] = 1:
///   67 & 1 = 1.
#[test]
fn issue_303_the_other_four_findings() {
    sim(
        "findings",
        r#"module M {
    in  p252 : u53
    in  p160 : i60
    in  p235 : i6
    in  p223 : u18
    in  p270 : i20
    in  p179 : u1
    in  p271 : u1
    in  p29  : u7
    in  p151 : u20
    out o191w : u57
    out o168w : u23
    out o190w : u3
    out o244w : u10
    o191w = (p252 / p252 & p160[54:2] as u53 & 3) as bits<53> as u53
    o168w = (p235 as u18 ^ 289327 - 27184 & p223) as bits<18> as u18
    o190w = (p270[17:17] as u1 | p179 & p271) as bits<1> as u1
    o244w = ((p29 | 65) & p151[10:4] as u7) as bits<7> as u7
}
test "first vector" {
    let dut = M { };
    dut.p252 = 5;
    dut.p160 = 4;
    dut.p235 = 31;
    dut.p223 = 0x2AAAA;
    dut.p270 = 0x20000;
    dut.p179 = 0;
    dut.p271 = 0;
    dut.p29 = 2;
    dut.p151 = 0x7F0;
    step(1);
    assert_eq(dut.o191w, 1);
    assert_eq(dut.o168w, 0x2AAB5);
    assert_eq(dut.o190w, 1);
    assert_eq(dut.o244w, 0x43);
}
test "second vector" {
    let dut = M { };
    dut.p252 = 0x1F_FFFF_FFFF_FFFF;
    dut.p160 = 0;
    dut.p235 = 0;
    dut.p223 = 0x3FFFF;
    dut.p270 = 0;
    dut.p179 = 1;
    dut.p271 = 0;
    dut.p29 = 2;
    dut.p151 = 0x10;
    step(1);
    assert_eq(dut.o191w, 0);
    assert_eq(dut.o168w, 0x3FFFF);
    assert_eq(dut.o190w, 0);
    assert_eq(dut.o244w, 1);
    dut.p271 = 1;
    step(1);
    assert_eq(dut.o190w, 1);
}
"#,
        2,
    );
}

/// Taramanın işaretsiz sınıfları, `e = a | b & c` (u8) üzerinde: iki ya
/// da üç aynı genişlik dönüşümü daha geniş hedefe (y1-y3), tipli `let`e
/// (y4), daha geniş toplamanın işleneni olarak (y5; type-inference.md §5:
/// işlem hedef tipte, u14) ve zincir bir bit işleminin işleneni olarak
/// (y6: `e as bits<8> as u8 & c`, `as` `&`'den sıkı bağlar).
/// a = 0x81, b = 0x3C, c = 0x0F: b & c = 0x0C, e = 0x8D; y1-y4 = 0x8D
/// (8-13. bitler 0); w = 0x1000: y5 = 0x108D; y6 = 0x8D & 0x0F = 0x0D.
/// a = b = c = 0xFF: e = 0xFF; w = 0x3F00: y5 = 0x3FFF; y6 = 0xFF.
#[test]
fn same_width_cast_chains_in_wider_unsigned_contexts() {
    sim(
        "unsigned",
        r#"module M {
    in  a  : u8
    in  b  : u8
    in  c  : u8
    in  w  : u14
    out y1 : u14
    out y2 : u14
    out y3 : u14
    out y4 : u14
    out y5 : u14
    out y6 : u14
    y1 = (a | (b & c)) as bits<8> as u8
    y2 = (a | (b & c)) as u8 as u8
    y3 = (a | (b & c)) as bits<8> as u8 as u8
    let x : u14 = (a | (b & c)) as u8 as u8
    y4 = x
    y5 = (a | (b & c)) as bits<8> as u8 + w
    y6 = ((a | (b & c)) as bits<8> as u8 & c) as bits<8> as u8
}
test "mixed bits" {
    let dut = M { };
    dut.a = 0x81;
    dut.b = 0x3C;
    dut.c = 0x0F;
    dut.w = 0x1000;
    step(1);
    assert_eq(dut.y1, 0x8D);
    assert_eq(dut.y2, 0x8D);
    assert_eq(dut.y3, 0x8D);
    assert_eq(dut.y4, 0x8D);
    assert_eq(dut.y5, 0x108D);
    assert_eq(dut.y6, 0x0D);
}
test "all ones" {
    let dut = M { };
    dut.a = 0xFF;
    dut.b = 0xFF;
    dut.c = 0xFF;
    dut.w = 0x3F00;
    step(1);
    assert_eq(dut.y1, 0xFF);
    assert_eq(dut.y2, 0xFF);
    assert_eq(dut.y3, 0xFF);
    assert_eq(dut.y4, 0xFF);
    assert_eq(dut.y5, 0x3FFF);
    assert_eq(dut.y6, 0xFF);
}
"#,
        2,
    );
}

/// Değeri genişliğe bağlı işlenen (`~`, `<<`; ADR-0106 §6) aynı genişlik
/// zinciri altında, daha geniş bağlamda: değer zincirin genişliğinde
/// hesaplanır, sonra genişletilir. Bu, ADR-0109'un iki A2 hariç
/// tutmasının (`emit_cast`, #303) dayandığı varsayımın kalıcı testidir:
/// aynı genişlikteki dönüşümün çıplak metnini daha geniş bağlama taşıyan
/// her yol onu yalıtır. Yalıtılmazsa SV işlenenleri hedef genişlikte
/// hesaplar ve değer değişir (sessiz yanlış).
/// type-inference.md §3.3/§3.4: `~` ve `<<` sol işlenenin tipinde kalır,
/// taşan bitler düşer. a = 0x13: ~a = 0xEC (14 bitte hesaplansa 0x3FEC);
/// a << 4 = 0x130'un alt 8 biti 0x30 (14 bitte 0x130); w = 0x100: y4 =
/// 0x130. a = 0xFF: ~a = 0; a << 4 = 0xF0 (14 bitte 0xFF0); y4 = 0x1F0.
/// s = 0x40 (64): s << 1 = 128'in i8 örüntüsü 0x80 = -128; i12'ye işaretle
/// genişler: 0xF80 (12 bitte hesaplansa 0x080). s = 0x20: 0x40.
#[test]
fn width_dependent_operands_under_same_width_chains_keep_their_width() {
    sim(
        "widthdep",
        r#"module M {
    in  a  : u8
    in  s  : i8
    in  w  : u14
    out y1 : u14
    out y2 : u14
    out y3 : u14
    out y4 : u14
    out z1 : i12
    out z2 : i12
    y1 = (~a) as u8 as u8
    y2 = (a << 4) as bits<8> as u8
    let x : u14 = (~a) as bits<8> as u8 as u8
    y3 = x
    y4 = (a << 4) as u8 as u8 + w
    z1 = (s << 1) as i8 as i12
    z2 = (s << 1) as i8 as i8
}
test "bits above the chain width stay zero" {
    let dut = M { };
    dut.a = 0x13;
    dut.w = 0x100;
    dut.s = 0x40;
    step(1);
    assert_eq(dut.y1, 0xEC);
    assert_eq(dut.y2, 0x30);
    assert_eq(dut.y3, 0xEC);
    assert_eq(dut.y4, 0x130);
    assert_eq(dut.z1, 0xF80);
    assert_eq(dut.z2, 0xF80);
}
test "all ones" {
    let dut = M { };
    dut.a = 0xFF;
    dut.w = 0x100;
    dut.s = 0x20;
    step(1);
    assert_eq(dut.y1, 0);
    assert_eq(dut.y2, 0xF0);
    assert_eq(dut.y3, 0);
    assert_eq(dut.y4, 0x1F0);
    assert_eq(dut.z1, 0x40);
    assert_eq(dut.z2, 0x40);
}
"#,
        2,
    );
}

/// Taramanın işaretli sınıfları, `e = s ^ 0x35 & t` (i8) üzerinde: aynı
/// genişlik dönüşümünden sonra genişletme (z1 `as i8 as i11`, z2 `as i8 as
/// i11 as i11` daha geniş hedefe), üç aynı genişlik dönüşümü (z5) ve daha
/// geniş toplama (z3) ve karşılaştırma (z4) içinde. Genişletme işaretle
/// yapılır (sv-mapping.md §6); çıkışlar ikiye tümleyen örüntüsüyle okunur.
/// s = 0x80 (-128), t = 0x7F: 0x35 & 0x7F = 0x35, e = 0x80 ^ 0x35 = 0xB5 =
/// -75. z1 (i11) = 2^11 - 75 = 0x7B5; z2 (i17) = 2^17 - 75 = 0x1FFB5;
/// z5 (i14) = 2^14 - 75 = 0x3FB5; ws = 100: z3 = 25 = 0x19; z4: -75 >
/// 100 yanlış. ws = 0x3FB4 (-76): -75 > -76 doğru.
/// s = 0x35, t = 0: e = 0x35 = 53, her genişlikte 0x35; ws = 100: z3 =
/// 153 = 0x99, z4 yanlış.
#[test]
fn same_width_cast_chains_in_wider_signed_contexts() {
    sim(
        "signed",
        r#"module M {
    in  s  : i8
    in  t  : i8
    in  ws : i14
    out z1 : i11
    out z2 : i17
    out z3 : i14
    out z4 : bool
    out z5 : i14
    z1 = (s ^ (0x35 & t)) as i8 as i11
    z2 = (s ^ (0x35 & t)) as i8 as i11 as i11
    z3 = (s ^ (0x35 & t)) as i8 as i8 + ws
    z4 = (s ^ (0x35 & t)) as i8 as i14 > ws
    z5 = (s ^ (0x35 & t)) as i8 as i8 as i8
}
test "negative value" {
    let dut = M { };
    dut.s = 0x80;
    dut.t = 0x7F;
    dut.ws = 100;
    step(1);
    assert_eq(dut.z1, 0x7B5);
    assert_eq(dut.z2, 0x1FFB5);
    assert_eq(dut.z3, 0x19);
    assert_eq(dut.z4, false);
    assert_eq(dut.z5, 0x3FB5);
    dut.ws = 0x3FB4;
    step(1);
    assert_eq(dut.z4, true);
}
test "positive value" {
    let dut = M { };
    dut.s = 0x35;
    dut.t = 0;
    dut.ws = 100;
    step(1);
    assert_eq(dut.z1, 0x35);
    assert_eq(dut.z2, 0x35);
    assert_eq(dut.z3, 0x99);
    assert_eq(dut.z4, false);
    assert_eq(dut.z5, 0x35);
}
"#,
        2,
    );
}

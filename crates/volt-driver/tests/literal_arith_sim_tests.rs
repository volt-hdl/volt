//! Yalnız literal içeren aritmetik derleme anında katlanır (#144),
//! davranış olarak, gerçek Verilator'da.
//!
//! Şartname: type-inference.md §3.3 (`(IntLit, IntLit) → IntLit`,
//! "Literal + Literal → hâlâ literal"), §4 (soneksiz literal beklenen
//! tipe uyarlanır, sığmazsa E2010) ve const-eval.md §1 (tam sayı `i128`
//! "geniş tutulur, sığdırma sonra kontrol edilir"), §4 (taşma hata, sarma
//! değil). Literal aritmetiğinin değeri bu yüzden tam değerdir: ara
//! değerler kesilmez, `/` ve `%` sıfıra doğru keser (`checked_div`,
//! `checked_rem`). Karşılaştırmada literal karşı yanın tipini alır
//! (ADR-0106 §1, §2).
//!
//! Matris #144'ün kapsamıdır: işleçler, işlenen biçimleri (iki literal,
//! generic parametre, `comb` içi `for` değişkeni) ve bağlamlar (atama,
//! tipli `let`, karşılaştırma). Düzeltmeden önce derleyici her literali
//! bağlam genişliğine kesip işlemi SV'ye bırakıyordu (`7 / 3` u2'de
//! `2'h3 / 2'd3` = 1). "Denetim" diye işaretli testler düzeltmeden önce
//! de geçiyordu (kesme o işlemle sıra değiştirir); kapsamın sınırını
//! gösterirler. Sığmayan sonuçlar (E2010) ve sıfıra bölme (E2023)
//! tanı testleri `tests/ui/fail/230`–`239`'da.
//!
//! Her test bir tasarımı `volt test` ile simüle eder; beklenen değerler
//! şartnameden elle hesaplandı (yorumlarda), derleyici çıktısından
//! alınmadı. Üretilen SV'nin metni karşılaştırılmaz; `volt test`
//! Verilator'ın WIDTH uyarılarında da durur, geçen test lint temizdir.
//! Test dilinin değerleri işaretsizdir: işaretli sonuç aynı genişlikte
//! `as uN` ile bit örüntüsü olarak okunur (ADR-0036).
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-litarith-{tag}-{}", std::process::id()));
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
    eprintln!("--- volt test (literal_arith_sim_tests::{tag})\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ İşleçler: iki literal, `out y : u2`, atama ═══════════════════════════

/// `7 / 3` = 2 (const-eval.md §4 `checked_div`, sıfıra doğru), u2'ye
/// sığar (0..=3). Düzeltmeden önce `2'h3 / 2'd3` = 1.
#[test]
fn issue_144_division_of_two_literals() {
    sim(
        "div",
        r#"module M {
    out y : u2
    y = 7 / 3
}
test "7 / 3 is 2" {
    let dut = M { };
    step(1);
    assert_eq(dut.y, 2);
}
"#,
        1,
    );
}

/// `5 % 3` = 2 (`checked_rem`). Düzeltmeden önce `2'h1 % 2'd3` = 1.
#[test]
fn issue_144_remainder_of_two_literals() {
    sim(
        "rem",
        r#"module M {
    out y : u2
    y = 5 % 3
}
test "5 % 3 is 2" {
    let dut = M { };
    step(1);
    assert_eq(dut.y, 2);
}
"#,
        1,
    );
}

/// Denetim: sonucu sığan `+`, `-`, `*` (2 + 1 = 3, 5 - 3 = 2, 3 * 1 = 3).
/// Kesme mod 4 bu işlemlerle sıra değiştirdiğinden düzeltmeden önce de
/// doğruydu; katlanmış değer aynı kalmalı.
#[test]
fn issue_144_fitting_add_sub_mul_keep_their_value() {
    sim(
        "addsubmul",
        r#"module M {
    out a : u2
    out s : u2
    out m : u2
    a = 2 + 1
    s = 5 - 3
    m = 3 * 1
}
test "fitting literal arithmetic" {
    let dut = M { };
    step(1);
    assert_eq(dut.a, 3);
    assert_eq(dut.s, 2);
    assert_eq(dut.m, 3);
}
"#,
        1,
    );
}

/// Ara değerler kesilmez (const-eval.md §1 `i128`): `100 * 3` = 300 hiçbir
/// dar tipe sığmaz ama `300 / 150` = 2 u2'ye sığar. İşaretli: `-9` i4'e
/// (-8..=7) sığmaz, `-9 / 2` = -4 (sıfıra doğru) ve `-9 % 4` = -1 (işaret
/// bölünenden) sığar; i4 örüntüleri 12 ve 15. Düzeltmeden önce sırasıyla
/// `2'h0 * 2'd3 / 2'h2` = 0, `4'sh7 / 4'sd2` = 3, `4'sh7 % 4'sd4` = 3.
#[test]
fn issue_144_intermediate_values_are_not_truncated() {
    sim(
        "wide",
        r#"module M {
    out y : u2
    out q : u4
    out r : u4
    y = 100 * 3 / 150
    let dq : i4 = -9 / 2
    let dr : i4 = -9 % 4
    q = dq as u4
    r = dr as u4
}
test "exact intermediate values" {
    let dut = M { };
    step(1);
    assert_eq(dut.y, 2);
    assert_eq(dut.q, 12);
    assert_eq(dut.r, 15);
}
"#,
        1,
    );
}

// ═══ İşlenen biçimleri (`/`, `out y : u2`) ════════════════════════════════

/// Generic parametre: monomorfizasyon `N`'yi 7 literaliyle değiştirir
/// (ADR-0041), `N / 3` iki literalin bölümüdür: 2. Düzeltmeden önce 1.
#[test]
fn issue_144_generic_parameter_divided_by_a_literal() {
    sim(
        "generic",
        r#"module G<const N: u32> {
    out y : u2
    y = N / 3
}
module Top {
    out y : u2
    let g = G<7> { }
    y = g.y
}
test "N / 3 with N = 7 is 2" {
    let dut = Top { };
    step(1);
    assert_eq(dut.y, 2);
}
"#,
        1,
    );
}

/// `comb` içi `for` değişkeni (const-eval.md §8): `i` = 7 iken `i / 3` = 2.
/// Düzeltmeden önce `2'h3 / 2'd3` = 1.
#[test]
fn issue_144_comb_for_variable_divided_by_a_literal() {
    sim(
        "combfor",
        r#"module M {
    out y : u2
    comb {
        for i in 7..8 {
            y = i / 3
        }
    }
}
test "i / 3 with i = 7 is 2" {
    let dut = M { };
    step(1);
    assert_eq(dut.y, 2);
}
"#,
        1,
    );
}

/// Denetim: modül düzeyi `for` değişkeni; açılım `2 / 2`'yi 1'e katlıyordu
/// (`fold_binary`, ADR-0056). u1'de 1.
#[test]
fn issue_144_module_level_for_variable_divided_by_a_literal() {
    sim(
        "modfor",
        r#"module M {
    out b : u1
    for i in 2..3 {
        b = i / 2
    }
}
test "i / 2 with i = 2 is 1" {
    let dut = M { };
    step(1);
    assert_eq(dut.b, 1);
}
"#,
        1,
    );
}

/// Denetim: tipli const (`B : u2 = 3`) tipli aritmetiktir, literal
/// aritmetiği değil: `B / 3` u2'de 1.
#[test]
fn issue_144_typed_const_divided_by_a_literal() {
    sim(
        "typedconst",
        r#"const B : u2 = 3

module M {
    out y : u2
    y = B / 3
}
test "B / 3 with B = 3 is 1" {
    let dut = M { };
    step(1);
    assert_eq(dut.y, 1);
}
"#,
        1,
    );
}

// ═══ Bağlamlar (`7 / 3`) ══════════════════════════════════════════════════

/// Tipli `let`: check kipinde u2, değer 2. Düzeltmeden önce 1.
#[test]
fn issue_144_typed_let_of_a_literal_division() {
    sim(
        "typedlet",
        r#"module M {
    out y : u2
    let t : u2 = 7 / 3
    y = t
}
test "let t : u2 = 7 / 3 is 2" {
    let dut = M { };
    step(1);
    assert_eq(dut.y, 2);
}
"#,
        1,
    );
}

/// Denetim: indeks konumu tam değerle katlanıyordu: `xs[7 / 3]` = xs[2].
#[test]
fn issue_144_literal_division_as_an_index() {
    sim(
        "index",
        r#"module M {
    in  x0 : u8
    in  x1 : u8
    in  x2 : u8
    in  x3 : u8
    out y : u8
    wire xs : [u8; 4]
    comb {
        xs[0] = x0
        xs[1] = x1
        xs[2] = x2
        xs[3] = x3
    }
    y = xs[7 / 3]
}
test "xs[7 / 3] is xs[2]" {
    let dut = M { };
    dut.x0 = 10;
    dut.x1 = 20;
    dut.x2 = 30;
    dut.x3 = 40;
    step(1);
    assert_eq(dut.y, 30);
}
"#,
        1,
    );
}

/// Karşılaştırma (ADR-0106 §1, §2): literal ifade karşı yanın tipini (u2)
/// alır. `x == 7 / 3` x = 2'de doğru, x = 1'de yanlış; `x < 5 % 3` (2)
/// x = 1'de doğru, x = 2'de yanlış. Düzeltmeden önce sağ yanlar 1'di.
#[test]
fn issue_144_comparison_with_literal_arithmetic() {
    sim(
        "compare",
        r#"module M {
    in  x : u2
    out e : bool
    out l : bool
    e = x == 7 / 3
    l = x < 5 % 3
}
test "the literal side is 2" {
    let dut = M { };
    dut.x = 2;
    step(1);
    assert_eq(dut.e, 1);
    assert_eq(dut.l, 0);
    dut.x = 1;
    step(1);
    assert_eq(dut.e, 0);
    assert_eq(dut.l, 1);
}
"#,
        1,
    );
}

/// Bit işlemi ve aritmetik işleneni: literal ifade somut yanın tipine
/// uyarlanır (§3.3). `x & (7 / 3)` x = 3'te 2; `x + 7 / 3` (u3) x = 1'de 3.
/// Düzeltmeden önce bit işleminde `x & 2'h3 / 2'd3` = 1.
#[test]
fn issue_144_literal_arithmetic_next_to_a_signal() {
    sim(
        "operand",
        r#"module M {
    in  x : u2
    out a : u2
    out s : u3
    a = x & (7 / 3)
    s = x + 7 / 3
}
test "the literal operand is 2" {
    let dut = M { };
    dut.x = 3;
    step(1);
    assert_eq(dut.a, 2);
    dut.x = 1;
    step(1);
    assert_eq(dut.s, 3);
}
"#,
        1,
    );
}

/// Derin literal ağacı da tam değeriyle katlanır: 70 terimlik
/// `1 + 1 + … + 1` = 70, `70 / 10` = 7 (const-eval.md §4 `checked_div`),
/// u3'e sığar (0..=7). Ağacın derinliği 70'tir; derinlik sınırı yoktur
/// (tip denetimi de ağacın tamamını katlar). Düzeltmeden önce 64'ten derin
/// ağaçta katlama bırakılıp eski yola dönülüyordu: 65 terimlik alt ağaç
/// bağlamın literali olarak `3'h1` (65 mod 8), bölen `3'h2` yazılıyordu;
/// `(1 + 5) / 2` = 3.
#[test]
fn issue_144_a_deep_literal_tree_is_folded_with_its_exact_value() {
    let ones = vec!["1"; 70].join(" + ");
    sim(
        "deep",
        &format!(
            r#"module M {{
    out y : u3
    y = ({ones}) / 10
}}
test "seventy ones divided by ten is 7" {{
    let dut = M {{ }};
    step(1);
    assert_eq(dut.y, 7);
}}
"#
        ),
        1,
    );
}

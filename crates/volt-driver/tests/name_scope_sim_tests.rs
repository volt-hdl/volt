//! Ad çözümü (#128) davranış olarak, gerçek Verilator'da: bir ad,
//! kullanıldığı yerin kapsam zincirinde çözülür (name-resolution.md §2,
//! §3.1, §6). Açılmış fn gövdesinin const'u, fn bildiriminin tip ve desen
//! ifadeleri ve const öğesinin değeri kök kapsamdadır (ADR-0081 Karar
//! 12.1): çağıranın aynı adlı döngü değişkeni, blok `let`'i ya da sinyali
//! onları yakalamaz. Blok kapsamında iç ad dıştakini gölgeler.
//!
//! Her test bir tasarımı `volt test` ile simüle eder; beklenen değerler
//! şartnameden elle hesaplandı (yorumlarda). Üretilen SV'nin metni
//! karşılaştırılmaz: `volt test` Verilator'ın WIDTH uyarılarında da
//! durur, yani geçen test lint temizdir de. Test dili dizi portu süremez;
//! diziler skaler girişlerden modülde kurulur, dizi çıkışları skaler
//! çıkışlara açılır.
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-namesim-{tag}-{}", std::process::id()));
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
    eprintln!("--- volt test (name_scope_sim_tests::{tag})\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ Açılmış fn gövdesinin const'u, çağıranın döngü değişkeni ══════════

/// #128: `pick`'in `I`'sı fn kapsamında köke, const'a bağlanır: her k için
/// `ys[k] = xs[1]` = 20. Hatada `xs[k]`: 10, 20, 30, 40.
const ISSUE_128: &str = r#"const I : u32 = 1
fn pick(x: [u8; 4]) -> u8 { x[I] }
module M {
    in  x0 : u8
    in  x1 : u8
    in  x2 : u8
    in  x3 : u8
    out y0 : u8
    out y1 : u8
    out y2 : u8
    out y3 : u8
    wire xs : [u8; 4]
    wire ys : [u8; 4]
    comb {
        xs[0] = x0
        xs[1] = x1
        xs[2] = x2
        xs[3] = x3
        for I in 0..4 { ys[I] = pick(xs) }
    }
    y0 = ys[0]
    y1 = ys[1]
    y2 = ys[2]
    y3 = ys[3]
}
test "the body reads the const" {
    let dut = M { };
    dut.x0 = 10;
    dut.x1 = 20;
    dut.x2 = 30;
    dut.x3 = 40;
    step(1);
    assert_eq(dut.y0, 20);
    assert_eq(dut.y1, 20);
    assert_eq(dut.y2, 20);
    assert_eq(dut.y3, 20);
}
"#;

#[test]
fn issue_128_a_function_const_in_an_index_is_not_the_callers_loop_variable() {
    sim("issue128", ISSUE_128, 1);
}

/// Hesaplanan indeks: `x[I + 1]` = `x[2]` = 30 her k için. Hatada
/// `xs[k + 1]`.
#[test]
fn a_function_const_in_a_computed_index_is_not_the_callers_loop_variable() {
    let src = ISSUE_128
        .replace("x[I] }", "x[I + 1] }")
        .replace("assert_eq(dut.y0, 20)", "assert_eq(dut.y0, 30)")
        .replace("assert_eq(dut.y1, 20)", "assert_eq(dut.y1, 30)")
        .replace("assert_eq(dut.y2, 20)", "assert_eq(dut.y2, 30)")
        .replace("assert_eq(dut.y3, 20)", "assert_eq(dut.y3, 30)");
    sim("computed", &src, 1);
}

/// Parça seçiminin başlangıcı: `a[I +: 2]` = a[2:1]; a = 6 (0b110) → 0b11
/// = 3 her k için. Hatada k = 0'da a[1:0] = 0b10 = 2.
const PART: &str = r#"const I : u32 = 1
fn sl(a: u8) -> u2 { a[I +: 2] as u2 }
module M {
    in  a  : u8
    out y0 : u2
    out y1 : u2
    wire ys : [u2; 2]
    comb { for I in 0..2 { ys[I] = sl(a) } }
    y0 = ys[0]
    y1 = ys[1]
}
test "the start is the const" {
    let dut = M { };
    dut.a = 6;
    step(1);
    assert_eq(dut.y0, 3);
    assert_eq(dut.y1, 3);
}
"#;

#[test]
fn a_function_const_as_a_part_select_start_is_not_the_callers_loop_variable() {
    sim("part", PART, 1);
}

/// Aralık sınırı: `a[I + 1:I]` = a[2:1] = 3 (a = 6). Hatada k = 0'da
/// a[1:0] = 2.
#[test]
fn a_function_const_as_a_range_bound_is_not_the_callers_loop_variable() {
    let src = PART.replace("a[I +: 2] as u2", "a[I + 1:I] as u2");
    sim("range", &src, 1);
}

/// Struct'ın genişliğe bağlı öğesi: `P { hi: K, lo: a }`, K u4 const 5;
/// ilk alan MSB'de (ADR-0077 Karar 3): a = 10 → 0x5A = 90. Hatada hi = k:
/// 10, 26.
const CONCAT: &str = r#"const K : u4 = 5
struct P { hi: u4, lo: u4 }
fn hi(a: u4) -> u8 { P { hi: K, lo: a } as u8 }
module M {
    in  a  : u4
    out y0 : u8
    out y1 : u8
    wire ys : [u8; 2]
    comb { for K in 0..2 { ys[K] = hi(a) } }
    y0 = ys[0]
    y1 = ys[1]
}
test "the field is the const" {
    let dut = M { };
    dut.a = 10;
    step(1);
    assert_eq(dut.y0, 90);
    assert_eq(dut.y1, 90);
}
"#;

#[test]
fn a_function_const_in_a_concatenation_keeps_its_declared_width() {
    sim("concat", CONCAT, 1);
}

/// Tip ifadesi (`uint<W>`) fn bildiriminde kök kapsamdadır: W = 4,
/// `inc(7)` = 8. Hatada genişlik döngü değişkeni (0) olur ve derleme
/// E2005 ile düşer.
const TYPE_WIDTH: &str = r#"const W : u32 = 4
fn inc(a: uint<W>) -> uint<W> { a + 1 }
module M {
    in  a  : u4
    out y0 : u4
    out y1 : u4
    wire ys : [u4; 2]
    comb { for W in 0..2 { ys[W] = inc(a) } }
    y0 = ys[0]
    y1 = ys[1]
}
test "the width is the const" {
    let dut = M { };
    dut.a = 7;
    step(1);
    assert_eq(dut.y0, 8);
    assert_eq(dut.y1, 8);
}
"#;

#[test]
fn a_function_type_width_names_the_const_not_the_callers_loop_variable() {
    sim("typewidth", TYPE_WIDTH, 1);
}

/// `let`'in tipi (`uint<W>`) çağıranın aynı adlı portuna bağlanmaz: W = 4,
/// `lo(0x5C)` = 0xC = 12, tel kipinde (`y`) ve ikame kipinde (`z`).
/// Port `W` modül kapsamında porttur: `wo = W`. Hatada E2005.
const LET_TYPE: &str = r#"const W : u32 = 4
fn lo(a: u8) -> u4 {
    let t : uint<W> = a[3:0] as u4
    t
}
module M {
    in  a  : u8
    in  W  : u8
    out y  : u4
    out z  : u4
    out wo : u8
    y = lo(a)
    comb { z = lo(a) }
    wo = W
}
test "the let type is the const" {
    let dut = M { };
    dut.a = 0x5C;
    dut.W = 200;
    step(1);
    assert_eq(dut.y, 12);
    assert_eq(dut.z, 12);
    assert_eq(dut.wo, 200);
}
"#;

#[test]
fn a_function_let_type_names_the_const_not_a_same_named_port() {
    sim("lettype", LET_TYPE, 1);
}

/// İç içe açılım: `outer` → `b`; `b`'nin `I`'sı yine const: `a[1]`,
/// a = 2 (0b0010) → doğru her k için. Hatada `a[k]`: yalnız k = 1 doğru.
const NESTED: &str = r#"const I : u32 = 1
fn b(x: u4) -> bool { x[I] }
fn outer(x: u4) -> bool { b(x) }
module M {
    in  a  : u4
    out y0 : bool
    out y1 : bool
    out y2 : bool
    out y3 : bool
    wire ys : [bool; 4]
    comb { for I in 0..4 { ys[I] = outer(a) } }
    y0 = ys[0]
    y1 = ys[1]
    y2 = ys[2]
    y3 = ys[3]
}
test "the nested body reads the const" {
    let dut = M { };
    dut.a = 2;
    step(1);
    assert_true(dut.y0);
    assert_true(dut.y1);
    assert_true(dut.y2);
    assert_true(dut.y3);
}
"#;

#[test]
fn a_nested_function_const_is_not_the_callers_loop_variable() {
    sim("nested", NESTED, 1);
}

/// Desendeki çıplak ad değerdir (§5.1), fn kapsamında const 1: s = 1 → 7
/// her k için, s = 2 → 0. Hatada desen `k`: s = 1'de yalnız k = 1 7 verir.
const PATTERN: &str = r#"const I : u2 = 1
fn cls(x: u2) -> u8 { match x { I => 7, _ => 0 } }
module M {
    in  s  : u2
    out y0 : u8
    out y1 : u8
    out y2 : u8
    out y3 : u8
    wire ys : [u8; 4]
    comb { for I in 0..4 { ys[I] = cls(s) } }
    y0 = ys[0]
    y1 = ys[1]
    y2 = ys[2]
    y3 = ys[3]
}
test "the pattern is the const" {
    let dut = M { };
    dut.s = 1;
    step(1);
    assert_eq(dut.y0, 7);
    assert_eq(dut.y1, 7);
    assert_eq(dut.y2, 7);
    assert_eq(dut.y3, 7);
    dut.s = 2;
    step(1);
    assert_eq(dut.y0, 0);
    assert_eq(dut.y1, 0);
    assert_eq(dut.y2, 0);
    assert_eq(dut.y3, 0);
}
"#;

#[test]
fn a_function_const_pattern_is_not_the_callers_loop_variable() {
    sim("pattern", PATTERN, 1);
}

/// Enum tipli const deseni: `M` = `Mode::B`; çağıranın `M` portu (başka
/// enum) değil. m = B → 1, m = A → 0. Hatada `m` port `M`'nin koduyla
/// karşılaştırılır: M = X (kod 0) iken m = A (kod 0) 1 verirdi.
const ENUM_PATTERN: &str = r#"enum Mode { A, B, C }
enum Other { X, Y, Z }
const M : Mode = Mode::B
fn sel(x: Mode) -> u8 { match x { M => 1, _ => 0 } }
module Top {
    in  m  : Mode
    in  M  : Other
    out y  : u8
    out mo : Other
    y = sel(m)
    mo = M
}
test "the pattern is the enum const" {
    let dut = Top { };
    dut.M = Other::X;
    dut.m = Mode::B;
    step(1);
    assert_eq(dut.y, 1);
    dut.m = Mode::A;
    step(1);
    assert_eq(dut.y, 0);
    assert_eq(dut.mo, Other::X);
}
"#;

#[test]
fn a_function_enum_const_pattern_is_not_a_same_named_port() {
    sim("enumpat", ENUM_PATTERN, 1);
}

/// Dizi sabiti: `T[2]` = 30, `T[i]` = T'nin i. elemanı; i = 3 → 40, i = 0
/// → 10. Hatada döngü değişkeni T'yi gölgeler ve derleme E2005 ile düşer.
const TABLE: &str = r#"const T : [u8; 4] = [10, 20, 30, 40]
fn lut() -> u8 { T[2] }
fn lutv(i: u2) -> u8 { T[i] }
module Top {
    in  i  : u2
    out y0 : u8
    out y1 : u8
    out z0 : u8
    out z1 : u8
    wire ys : [u8; 2]
    wire zs : [u8; 2]
    comb { for T in 0..2 { ys[T] = lut()
                           zs[T] = lutv(i) } }
    y0 = ys[0]
    y1 = ys[1]
    z0 = zs[0]
    z1 = zs[1]
}
test "the table is the const" {
    let dut = Top { };
    dut.i = 3;
    step(1);
    assert_eq(dut.y0, 30);
    assert_eq(dut.y1, 30);
    assert_eq(dut.z0, 40);
    assert_eq(dut.z1, 40);
    dut.i = 0;
    step(1);
    assert_eq(dut.z1, 10);
}
"#;

#[test]
fn a_function_const_array_is_not_the_callers_loop_variable() {
    sim("table", TABLE, 1);
}

/// Dizi sabiti, çağıranın aynı adlı dizi sinyali: fn gövdesinde `T` const
/// (`T[3]` = 40, `T[2]` = 30), modülde `T` sinyal (`t3 = T[3]` = 4).
/// Hatada derleme E2005 ile düşer.
const TABLE_SIGNAL: &str = r#"const T : [u8; 4] = [10, 20, 30, 40]
fn lutv(i: u2) -> u8 { T[i] }
fn lut() -> u8 { T[2] }
module Top {
    in  i  : u2
    in  x0 : u8
    in  x1 : u8
    in  x2 : u8
    in  x3 : u8
    out y  : u8
    out z  : u8
    out t3 : u8
    wire T : [u8; 4]
    comb {
        T[0] = x0
        T[1] = x1
        T[2] = x2
        T[3] = x3
    }
    y = lutv(i)
    z = lut()
    t3 = T[3]
}
test "the body reads the const, the module the signal" {
    let dut = Top { };
    dut.x0 = 1;
    dut.x1 = 2;
    dut.x2 = 3;
    dut.x3 = 4;
    dut.i = 3;
    step(1);
    assert_eq(dut.y, 40);
    assert_eq(dut.z, 30);
    assert_eq(dut.t3, 4);
}
"#;

#[test]
fn a_function_const_array_is_not_a_same_named_signal() {
    sim("tablesig", TABLE_SIGNAL, 1);
}

/// Aralık dış kapsamda (CE §8): `0..I`, I = 2, iki yineleme; gövdedeki
/// çağrı yine const: `ys[k] = xs[2]` = 30 (k = 0, 1), ys[2..3] önceki
/// `ys = xs`: 30, 40.
const LOOP_RANGE: &str = r#"const I : u32 = 2
fn pick(x: [u8; 4]) -> u8 { x[I] }
module M {
    in  x0 : u8
    in  x1 : u8
    in  x2 : u8
    in  x3 : u8
    out y0 : u8
    out y1 : u8
    out y2 : u8
    out y3 : u8
    wire xs : [u8; 4]
    wire ys : [u8; 4]
    comb {
        xs[0] = x0
        xs[1] = x1
        xs[2] = x2
        xs[3] = x3
        ys = xs
        for I in 0..I { ys[I] = pick(xs) }
    }
    y0 = ys[0]
    y1 = ys[1]
    y2 = ys[2]
    y3 = ys[3]
}
test "two iterations read the const" {
    let dut = M { };
    dut.x0 = 10;
    dut.x1 = 20;
    dut.x2 = 30;
    dut.x3 = 40;
    step(1);
    assert_eq(dut.y0, 30);
    assert_eq(dut.y1, 30);
    assert_eq(dut.y2, 30);
    assert_eq(dut.y3, 40);
}
"#;

#[test]
fn a_loop_range_names_the_const_its_variable_shadows() {
    sim("looprange", LOOP_RANGE, 1);
}

/// Const'un değeri kök kapsamda değerlendirilir (NR §4): `J = I + 1` = 2,
/// kullanıldığı döngünün `I`'sı onu değiştirmez: `xs[2]` = 30 her k için.
/// Hatada `xs[k + 1]`: 20, 30, 40, x.
#[test]
fn a_const_value_is_evaluated_in_the_root_scope() {
    let src = ISSUE_128
        .replace(
            "const I : u32 = 1\nfn pick(x: [u8; 4]) -> u8 { x[I] }",
            "const I : u32 = 1\nconst J : u32 = I + 1",
        )
        .replace("ys[I] = pick(xs)", "ys[I] = xs[J]")
        .replace("assert_eq(dut.y0, 20)", "assert_eq(dut.y0, 30)")
        .replace("assert_eq(dut.y1, 20)", "assert_eq(dut.y1, 30)")
        .replace("assert_eq(dut.y2, 20)", "assert_eq(dut.y2, 30)")
        .replace("assert_eq(dut.y3, 20)", "assert_eq(dut.y3, 30)");
    sim("constvalue", &src, 1);
}

/// Gövdedeki `K` const 5, çağıranın aynı adlı portu (9) değil: `f(10)` =
/// 15. (Bekçi: değer düzeltmeden önce de doğruydu, #128'in 21. hücresi.)
const CONST_BESIDE_PORT: &str = r#"const K : u4 = 5
fn f(a: u8) -> u8 { a + K }
module Top {
    in  a  : u8
    in  K  : u4
    out y  : u8
    out ko : u4
    y = f(a)
    ko = K
}
test "the body adds the const" {
    let dut = Top { };
    dut.a = 10;
    dut.K = 9;
    step(1);
    assert_eq(dut.y, 15);
    assert_eq(dut.ko, 9);
}
"#;

#[test]
fn a_function_const_beside_a_same_named_port_is_the_const() {
    sim("constport", CONST_BESIDE_PORT, 1);
}

// ═══ Blok kapsamı: iç ad dıştakini gölgeler (§2, §6) ═══════════════════

/// Dört girişten kurulan `xs` ile bir modül; `body` çıkışları ve `comb`'u.
fn with_xs(head: &str, body: &str, test: &str) -> String {
    format!(
        "{head}module M {{
    in  x0 : u8
    in  x1 : u8
    in  x2 : u8
    in  x3 : u8
    wire xs : [u8; 4]
    comb {{
        xs[0] = x0
        xs[1] = x1
        xs[2] = x2
        xs[3] = x3
    }}
{body}
}}
test \"block scope\" {{
    let dut = M {{ }};
    dut.x0 = 10;
    dut.x1 = 20;
    dut.x2 = 30;
    dut.x3 = 40;
{test}
}}
"
    )
}

/// Blok `let`'i const'u gölgeler: `xs[K + 0]`, K yerel = s = 1 → 20.
/// Hatada const 3 → 40.
#[test]
fn a_block_let_shadows_a_const_in_a_computed_index() {
    let src = with_xs(
        "const K : u32 = 3\n",
        "    in  s : u2
    out y : u8
    comb { let K = s
           y = xs[K + 0] }",
        "    dut.s = 1;
    step(1);
    assert_eq(dut.y, 20);",
    );
    sim("blocklet", &src, 1);
}

/// Modül sinyali const'u gölgeler: `xs[K + 0]`, K port = 1 → 20 (bekçi).
#[test]
fn a_module_signal_shadows_a_const_in_a_computed_index() {
    let src = with_xs(
        "const K : u32 = 3\n",
        "    in  K : u2
    out y : u8
    y = xs[K + 0]",
        "    dut.K = 1;
    step(1);
    assert_eq(dut.y, 20);",
    );
    sim("modsig", &src, 1);
}

/// Döngü değişkeni aynı adlı portu (i = 3) gölgeler: `ys[k] = xs[k]`,
/// değer konumunda `zs[k] = k`, hesaplanan indekste `ws[k] = xs[k + 0]`.
/// Hatada `zs` hep 3, `ws` hep 40.
#[test]
fn a_loop_variable_shadows_a_same_named_port() {
    let src = with_xs(
        "",
        "    in  i  : u2
    out y0 : u8
    out y3 : u8
    out z0 : u2
    out z1 : u2
    out z2 : u2
    out z3 : u2
    out w0 : u8
    out w1 : u8
    out w2 : u8
    out w3 : u8
    wire ys : [u8; 4]
    wire zs : [u2; 4]
    wire ws : [u8; 4]
    comb { for i in 0..4 { ys[i] = xs[i]
                           zs[i] = i
                           ws[i] = xs[i + 0] } }
    y0 = ys[0]
    y3 = ys[3]
    z0 = zs[0]
    z1 = zs[1]
    z2 = zs[2]
    z3 = zs[3]
    w0 = ws[0]
    w1 = ws[1]
    w2 = ws[2]
    w3 = ws[3]",
        "    dut.i = 3;
    step(1);
    assert_eq(dut.y0, 10);
    assert_eq(dut.y3, 40);
    assert_eq(dut.z0, 0);
    assert_eq(dut.z1, 1);
    assert_eq(dut.z2, 2);
    assert_eq(dut.z3, 3);
    assert_eq(dut.w0, 10);
    assert_eq(dut.w1, 20);
    assert_eq(dut.w2, 30);
    assert_eq(dut.w3, 40);",
    );
    sim("loopport", &src, 1);
}

/// Döngü gövdesindeki blok `let`'i döngü değişkenini gölgeler: `xs[i]`
/// yereldir (s = 2) → 30. Hatada son yinelemede `xs[1]` = 20.
#[test]
fn a_block_let_in_a_loop_body_shadows_the_loop_variable() {
    let src = with_xs(
        "",
        "    in  s  : u2
    out y0 : u8
    out y1 : u8
    wire ys : [u8; 2]
    comb { for i in 0..2 { let i = s
                           ys[0] = xs[i] }
           ys[1] = 0 }
    y0 = ys[0]
    y1 = ys[1]",
        "    dut.s = 2;
    step(1);
    assert_eq(dut.y0, 30);
    assert_eq(dut.y1, 0);",
    );
    sim("loopbodylet", &src, 1);
}

/// Döngüden önce bildirilen blok `let`'i döngü değişkeninin dışındadır:
/// döngüde `i` döngü değişkenidir (`ys = [10, 20]`), döngüden sonra yine
/// yerel (s = 3 → 40). (Bekçi.)
#[test]
fn a_loop_variable_shadows_an_outer_block_let() {
    let src = with_xs(
        "",
        "    in  s  : u2
    out y0 : u8
    out y1 : u8
    out y  : u8
    wire ys : [u8; 2]
    comb { let i = s
           for i in 0..2 { ys[i] = xs[i] }
           y = xs[i] }
    y0 = ys[0]
    y1 = ys[1]",
        "    dut.s = 3;
    step(1);
    assert_eq(dut.y0, 10);
    assert_eq(dut.y1, 20);
    assert_eq(dut.y, 40);",
    );
    sim("outerlet", &src, 1);
}

/// W1002 veren her gölgelemenin anlamı (name_hygiene_tests'in tasarımı):
/// fn parametresi `I` (= 2) ve fn `let`'i `I` (= 2) kök const'u gölgeler →
/// `xs[2]` = 30; blok `let`'i `K` = s = 1 → `xs[1]` = 20; döngü gövdesinin
/// `let i` = s döngü değişkenini gölgeler: yalnız `ys[1]` yazılır,
/// `by_root`'un `I`'sı const 1 → 20; modül düzeyinde `i` porttur (3) → 40.
#[test]
fn every_shadowing_declaration_reads_its_own_binding() {
    let src = with_xs(
        "const I : u32 = 1
const K : u32 = 3
fn by_param(x: [u8; 4], I: u2) -> u8 { x[I] }
fn by_let(x: [u8; 4]) -> u8 {
    let I : u2 = 2
    x[I]
}
fn by_root(x: [u8; 4]) -> u8 { x[I] }
",
        "    in  s  : u2
    in  i  : u2
    out a  : u8
    out b  : u8
    out c  : u8
    out d  : u8
    out y0 : u8
    out y1 : u8
    wire ys : [u8; 4]
    comb {
        for k in 0..4 { ys[k] = 0 }
        let K = s
        a = xs[K]
        for i in 0..4 {
            let i = s
            ys[i] = by_root(xs)
        }
    }
    b = by_param(xs, 2)
    c = by_let(xs)
    d = xs[i]
    y0 = ys[0]
    y1 = ys[1]",
        "    dut.s = 1;
    dut.i = 3;
    step(1);
    assert_eq(dut.a, 20);
    assert_eq(dut.b, 30);
    assert_eq(dut.c, 30);
    assert_eq(dut.d, 40);
    assert_eq(dut.y0, 0);
    assert_eq(dut.y1, 20);",
    );
    sim("w1002", &src, 1);
}

// ═══ Sabit değerlendirmenin derinlik sınırı ═══════════════════════════

/// 63 halkalı enum const zinciri (`A0 = A1`, …, `A62 = E::Y`) match'in
/// sınananında katlanır: E::Y kolu, N = 2. Derinlik 0'dan sayılır (N'nin
/// match'i 0, `A{j}` j + 1, `E::Y` 64): sınır 64. 64 halkada E0003
/// (volt-sv-emit `name_scope_tests`).
#[test]
fn an_enum_const_chain_of_63_links_folds() {
    let mut src = String::from("enum E { X, Y }\n");
    for j in 0..63 {
        let value = if j == 62 {
            "E::Y".to_string()
        } else {
            format!("A{}", j + 1)
        };
        src.push_str(&format!("const A{j} : E = {value}\n"));
    }
    src.push_str(
        r#"const N : u8 = match A0 { E::X => 1, E::Y => 2 }
module M {
    in  a : u8
    out o : u8
    o = a + N
}
test "the chain folds" {
    let dut = M { };
    dut.a = 40;
    step(1);
    assert_eq(dut.o, 42);
}
"#,
    );
    sim("chain63", &src, 1);
}

// ═══ Sabit indeks ═════════════════════════════════════════════════════

/// Sabit indeks ve parça seçimi başlangıcı (`K : u32` = 2): a = 0x34
/// (0011_0100) → `a[2]` = 1, `a[2 +: 4]` = bitler 5..2 = 0b1101 = 13.
/// Tipi adres genişliğinden farklı ve 32 bit olmayan sabit indeks
/// Verilator'da WIDTHTRUNC verir: #134.
const CONST_INDEX: &str = r#"const K : u32 = 2
module M {
    in  a : u8
    out o : bool
    out y : bits<4>
    o = a[K]
    y = a[K +: 4]
}
test "a constant index selects its bit" {
    let dut = M { };
    dut.a = 0x34;
    step(1);
    assert_true(dut.o);
    assert_eq(dut.y, 13);
}
"#;

#[test]
fn a_constant_index_selects_its_bit() {
    sim("constindex", CONST_INDEX, 1);
}

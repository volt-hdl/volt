//! Ayrıştırıcının ikamesi kapsam zincirinden geçer (#130), davranış
//! olarak, gerçek Verilator'da. Generic monomorfizasyonu (ADR-0041) ve
//! modül düzeyi `for` açılımı (ADR-0056) ad çözümünden önce koşar; bir
//! ad yine de kullanıldığı yerin kapsam zincirinde çözülür
//! (name-resolution.md §2, §4, §6): iç kapsamda yeniden bildirilen ad
//! (blok `let`'i, iç döngü değişkeni, döngü gövdesinin `let`'i) dıştaki
//! generic parametreyi ya da döngü değişkenini gölgeler, kullanım iç
//! bildirime bağlanır.
//!
//! Her test bir tasarımı `volt test` ile simüle eder; beklenen değerler
//! şartnameden elle hesaplandı (yorumlarda), derleyici çıktısından
//! alınmadı. Üretilen SV'nin metni karşılaştırılmaz: `volt test`
//! Verilator'ın WIDTH uyarılarında da durur, yani geçen test lint
//! temizdir de. Test dili dizi portu süremez; diziler skaler girişlerden
//! modülde kurulur, dizi çıkışları skaler çıkışlara açılır. Tanılar
//! (W1002, E-kodları) `parser_scope_diag_tests`'te.
//!
//! Verilator yoksa testler atlanır; `VOLT_REQUIRE_TOOLS=verilator` ile
//! (CI'ın araçlı işi) yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-parserscope-{tag}-{}", std::process::id()));
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
    eprintln!("--- volt test (parser_scope_sim_tests::{tag})\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Dört skaler girişten kurulan `xs : [u8; 4]` (x0..x3).
const XS_PORTS: &str = "    in  x0 : u8
    in  x1 : u8
    in  x2 : u8
    in  x3 : u8
";
const XS_WIRE: &str = "    wire xs : [u8; 4]
    comb {
        xs[0] = x0
        xs[1] = x1
        xs[2] = x2
        xs[3] = x3
    }
";
/// Test bloğunda x0..x3 = 10, 20, 30, 40.
const XS_DRIVE: &str = "    dut.x0 = 10;
    dut.x1 = 20;
    dut.x2 = 30;
    dut.x3 = 40;
";
/// `ys : [u8; 4]`'ün skaler çıkışları y0..y3.
const YS_PORTS: &str = "    out y0 : u8
    out y1 : u8
    out y2 : u8
    out y3 : u8
";
const YS_WIRE: &str = "    wire ys : [u8; 4]
";
const YS_OUT: &str = "    y0 = ys[0]
    y1 = ys[1]
    y2 = ys[2]
    y3 = ys[3]
";

fn asserts4(vals: [u32; 4]) -> String {
    vals.iter()
        .enumerate()
        .map(|(k, v)| format!("    assert_eq(dut.y{k}, {v});\n"))
        .collect()
}

// ═══ Generic parametre (G<3>) ═══════════════════════════════════════════

/// Denetim 1 (#130 satır 34): blok `let N` generic `N`'yi gölgeler
/// (§2 blok kapsamı modülün içinde, §6). `y = N` yerel `N`'yi, yani
/// `s`'yi okur: s = 42 → 42, s = 7 → 7. Hatada her zaman 3.
const G_BLOCK_LET: &str = r#"module G<const N: u32> {
    in  s : u8
    out y : u8
    comb {
        let N = s
        y = N
    }
}
module Top {
    in  s : u8
    out y : u8
    let g = G<3> { s: s }
    y = g.y
}
test "the block let shadows the generic parameter" {
    let dut = Top { };
    dut.s = 42;
    step(1);
    assert_eq(dut.y, 42);
    dut.s = 7;
    step(1);
    assert_eq(dut.y, 7);
}
"#;

#[test]
fn issue_130_a_block_let_shadows_the_generic_parameter() {
    sim("g_let", G_BLOCK_LET, 1);
}

/// Denetim 2 (#130 satır 35): `comb` içi `for N` generic `N`'yi gölgeler;
/// döngü kapsamında `ys[N] = N` her k için `ys[k] = k`: y0 = 0, y1 = 1.
/// Hatada `ys[3]` (E2006, E4012).
#[test]
fn issue_130_a_comb_loop_variable_shadows_the_generic_parameter() {
    let src = r#"module G<const N: u32> {
    out y0 : u8
    out y1 : u8
    wire ys : [u8; 2]
    comb { for N in 0..2 { ys[N] = N } }
    y0 = ys[0]
    y1 = ys[1]
}
module Top {
    out y0 : u8
    out y1 : u8
    let g = G<3> { }
    y0 = g.y0
    y1 = g.y1
}
test "the loop variable shadows the generic parameter" {
    let dut = Top { };
    step(1);
    assert_eq(dut.y0, 0);
    assert_eq(dut.y1, 1);
}
"#;
    sim("g_combfor", src, 1);
}

/// Denetim 3: blok `let N` indekste: `xs[N]` = `xs[s]`. s = 1 → 20,
/// s = 2 → 30. Hatada `xs[3]` = 40.
#[test]
fn a_block_let_that_shadows_the_generic_is_the_index() {
    let src = format!(
        r#"module G<const N: u32> {{
{XS_PORTS}    in  s  : u2
    out y  : u8
{XS_WIRE}    comb {{
        let N = s
        y = xs[N]
    }}
}}
module Top {{
{XS_PORTS}    in  s  : u2
    out y  : u8
    let g = G<3> {{ x0: x0, x1: x1, x2: x2, x3: x3, s: s }}
    y = g.y
}}
test "the index is the local" {{
    let dut = Top {{ }};
{XS_DRIVE}    dut.s = 1;
    step(1);
    assert_eq(dut.y, 20);
    dut.s = 2;
    step(1);
    assert_eq(dut.y, 30);
}}
"#
    );
    sim("g_index", &src, 1);
}

/// Denetim 4: parça seçiminin başlangıcı `a[N +: 2]` = `a[s +: 2]`.
/// a = 182 (0b1011_0110): s = 1 → (182 >> 1) & 3 = 3; s = 5 → (182 >> 5)
/// & 3 = 1. Hatada başlangıç 3: (182 >> 3) & 3 = 2.
#[test]
fn a_block_let_that_shadows_the_generic_is_the_part_select_start() {
    let src = r#"module G<const N: u32> {
    in  a : u8
    in  s : u3
    out y : u2
    comb {
        let N = s
        y = a[N +: 2] as u2
    }
}
module Top {
    in  a : u8
    in  s : u3
    out y : u2
    let g = G<3> { a: a, s: s }
    y = g.y
}
test "the start is the local" {
    let dut = Top { };
    dut.a = 182;
    dut.s = 1;
    step(1);
    assert_eq(dut.y, 3);
    dut.s = 5;
    step(1);
    assert_eq(dut.y, 1);
}
"#;
    sim("g_part", src, 1);
}

/// Denetim 6: `comb` içi `for N in 0..4 { ys[N] = xs[N] }`: her k için
/// `ys[k] = xs[k]`: 10, 20, 30, 40. Hatada dört yazım da `ys[3]`'e (E4012).
#[test]
fn a_comb_loop_variable_that_shadows_the_generic_indexes_both_sides() {
    let src = format!(
        r#"module G<const N: u32> {{
{XS_PORTS}{YS_PORTS}{XS_WIRE}{YS_WIRE}    comb {{ for N in 0..4 {{ ys[N] = xs[N] }} }}
{YS_OUT}}}
module Top {{
{XS_PORTS}{YS_PORTS}    let g = G<3> {{ x0: x0, x1: x1, x2: x2, x3: x3 }}
    y0 = g.y0
    y1 = g.y1
    y2 = g.y2
    y3 = g.y3
}}
test "each element is copied" {{
    let dut = Top {{ }};
{XS_DRIVE}    step(1);
{}}}
"#,
        asserts4([10, 20, 30, 40])
    );
    sim("g_combfor_index", &src, 1);
}

/// Denetim 7: modül düzeyi `for N` generic `N`'yi gölgeler (ADR-0056
/// açılımı döngünün kapsamında çözer): `ys[k] = k`: y0 = 0, y1 = 1.
#[test]
fn a_module_level_loop_variable_shadows_the_generic_parameter() {
    let src = r#"module G<const N: u32> {
    out y0 : u8
    out y1 : u8
    wire ys : [u8; 2]
    for N in 0..2 { ys[N] = N }
    y0 = ys[0]
    y1 = ys[1]
}
module Top {
    out y0 : u8
    out y1 : u8
    let g = G<3> { }
    y0 = g.y0
    y1 = g.y1
}
test "each iteration writes its own element" {
    let dut = Top { };
    step(1);
    assert_eq(dut.y0, 0);
    assert_eq(dut.y1, 1);
}
"#;
    sim("g_modfor", src, 1);
}

/// Alt modülün generic argümanı: `W<K>` `K`'yi çıkarır.
const W_MODULE: &str = "module W<const K: u32> {
    out y : u8
    y = K
}
";

/// Denetim 8: modül düzeyi `for N`'nin gövdesindeki `W<N>` döngü
/// değişkenidir: W<0>, W<1> → y0 = 0, y1 = 1. Hatada iki kez W<3>.
#[test]
fn a_generic_argument_names_the_loop_variable_that_shadows_the_generic() {
    let src = format!(
        r#"{W_MODULE}module G<const N: u32> {{
    out y0 : u8
    out y1 : u8
    wire ys : [u8; 2]
    for N in 0..2 {{
        let w = W<N> {{ }}
        ys[N] = w.y
    }}
    y0 = ys[0]
    y1 = ys[1]
}}
module Top {{
    out y0 : u8
    out y1 : u8
    let g = G<3> {{ }}
    y0 = g.y0
    y1 = g.y1
}}
test "each instance gets the iteration value" {{
    let dut = Top {{ }};
    step(1);
    assert_eq(dut.y0, 0);
    assert_eq(dut.y1, 1);
}}
"#
    );
    sim("g_modfor_arg", &src, 1);
}

/// Denetim 9 (bekçi): iç içe generic örnekleme. `A<3>` içinde `B<N>`
/// A'nın N'sidir (3); B'nin kendi N'si B'nin modül kapsamında: y = 3.
#[test]
fn a_nested_generic_instantiation_receives_the_outer_value() {
    let src = r#"module B<const N: u32> {
    out y : u8
    y = N
}
module A<const N: u32> {
    out y : u8
    let b = B<N> { }
    y = b.y
}
module Top {
    out y : u8
    let a = A<3> { }
    y = a.y
}
test "the inner module sees the outer argument" {
    let dut = Top { };
    step(1);
    assert_eq(dut.y, 3);
}
"#;
    sim("g_nested", src, 1);
}

/// Denetim 10–12 (bekçi): satır içi açılan fn'in parametresi ve `let`'i
/// fn kapsamında (ebeveyni kök, ADR-0081 §1), kök const'u fn gövdesinde
/// (ADR-0081 K12.1); generic modülün `N`'si fn gövdesine girmez.
/// `f(N) = N ^ 1`, `g(a) = { let N = a ^ 2; N }`, `pick(x) = x[N]`
/// (const N = 1): s = 40 → y = 41, z = 42; w = xs[1] = 20.
#[test]
fn function_parameters_lets_and_consts_ignore_the_callers_generic() {
    let src = format!(
        r#"const N : u32 = 1
fn f(N: u8) -> u8 {{ N ^ 1 }}
fn g(a: u8) -> u8 {{
    let N = a ^ 2
    N
}}
fn pick(x: [u8; 4]) -> u8 {{ x[N] }}
module G<const N: u32> {{
    in  s : u8
{XS_PORTS}    out y : u8
    out z : u8
    out w : u8
{XS_WIRE}    y = f(s)
    z = g(s)
    w = pick(xs)
}}
module Top {{
    in  s : u8
{XS_PORTS}    out y : u8
    out z : u8
    out w : u8
    let u = G<3> {{ s: s, x0: x0, x1: x1, x2: x2, x3: x3 }}
    y = u.y
    z = u.z
    w = u.w
}}
test "the function scope is its own" {{
    let dut = Top {{ }};
    dut.s = 40;
{XS_DRIVE}    step(1);
    assert_eq(dut.y, 41);
    assert_eq(dut.z, 42);
    assert_eq(dut.w, 20);
}}
"#
    );
    sim("g_fn", &src, 1);
}

/// Denetim 13 (bekçi): desendeki generic parametre değer desenidir
/// (§5.1): `N => 1` ≡ `s == 3`. s = 3 → 1, s = 4 → 0.
#[test]
fn a_generic_parameter_in_a_pattern_is_its_value() {
    let src = r#"module G<const N: u32> {
    in  s : u8
    out y : u8
    y = match s {
        N => 1,
        _ => 0,
    }
}
module Top {
    in  s : u8
    out y : u8
    let g = G<3> { s: s }
    y = g.y
}
test "the arm compares with the argument" {
    let dut = Top { };
    dut.s = 3;
    step(1);
    assert_eq(dut.y, 1);
    dut.s = 4;
    step(1);
    assert_eq(dut.y, 0);
}
"#;
    sim("g_pattern", src, 1);
}

/// Denetim 15 (#122): generic parametre kök const'u gölgeler (W1002);
/// gövdede `N` parametredir: y = 3, const'un 7'si değil.
#[test]
fn a_generic_parameter_that_shadows_a_root_const_is_the_argument() {
    let src = r#"const N : u32 = 7
module G<const N: u32> {
    out y : u8
    y = N
}
module Top {
    out y : u8
    let g = G<3> { }
    y = g.y
}
test "the parameter wins" {
    let dut = Top { };
    step(1);
    assert_eq(dut.y, 3);
}
"#;
    sim("g_root_const", src, 1);
}

/// Denetim 18: kısaltılmış port bağlaması `Sub { N }` ≡ `Sub { N: N }`;
/// değer generic parametredir: y = 3. Hatada E1001.
#[test]
fn a_shorthand_port_binding_reads_the_generic_parameter() {
    let src = r#"module Sub {
    in  N : u8
    out y : u8
    y = N
}
module G<const N: u32> {
    out y : u8
    let u = Sub { N }
    y = u.y
}
module Top {
    out y : u8
    let g = G<3> { }
    y = g.y
}
test "the port gets the argument" {
    let dut = Top { };
    step(1);
    assert_eq(dut.y, 3);
}
"#;
    sim("g_shorthand", src, 1);
}

/// Denetim 37–38: blok `let`'i yalnız kendi bloğunda ve bildiriminden
/// sonra görünür (ADR-0083 K8). `if c { let N = s; y = N } else { y = N }`:
/// c = 1 → s = 42, c = 0 → 3. Sıralı: `y0 = N; let N = s; y1 = N` →
/// y0 = 3, y1 = s = 42.
#[test]
fn a_block_let_shadows_the_generic_only_in_its_block_and_after_it() {
    let src = r#"module G<const N: u32> {
    in  c  : bool
    in  s  : u8
    out y  : u8
    out y0 : u8
    out y1 : u8
    comb {
        if c {
            let N = s
            y = N
        } else {
            y = N
        }
    }
    comb {
        y0 = N
        let N = s
        y1 = N
    }
}
module Top {
    in  c  : bool
    in  s  : u8
    out y  : u8
    out y0 : u8
    out y1 : u8
    let g = G<3> { c: c, s: s }
    y = g.y
    y0 = g.y0
    y1 = g.y1
}
test "scope ends with the block" {
    let dut = Top { };
    dut.s = 42;
    dut.c = true;
    step(1);
    assert_eq(dut.y, 42);
    assert_eq(dut.y0, 3);
    assert_eq(dut.y1, 42);
    dut.c = false;
    step(1);
    assert_eq(dut.y, 3);
}
"#;
    sim("g_block_extent", src, 1);
}

/// Denetim 34 (bekçi): modül düzeyi `for`'un sınırı generic parametre
/// (`0..N`, N = 4): dört yineleme, `ys = xs`.
#[test]
fn a_module_level_loop_bound_is_the_generic_argument() {
    let src = format!(
        r#"module G<const N: u32> {{
{XS_PORTS}{YS_PORTS}{XS_WIRE}{YS_WIRE}    for i in 0..N {{ ys[i] = xs[i] }}
{YS_OUT}}}
module Top {{
{XS_PORTS}{YS_PORTS}    let g = G<4> {{ x0: x0, x1: x1, x2: x2, x3: x3 }}
    y0 = g.y0
    y1 = g.y1
    y2 = g.y2
    y3 = g.y3
}}
test "four iterations" {{
    let dut = Top {{ }};
{XS_DRIVE}    step(1);
{}}}
"#,
        asserts4([10, 20, 30, 40])
    );
    sim("g_bound", &src, 1);
}

// ═══ Modül düzeyi `for` değişkeni ══════════════════════════════════════

/// Denetim 19 (#130 satır 36): döngü gövdesinin `let i`'si döngü
/// değişkenini gölgeler (gövde döngünün içinde bir blok kapsamı, §2):
/// `y = i` yerel `i`'yi (`s`) okur: 42. Hatada 0.
#[test]
fn issue_130_a_body_let_shadows_the_loop_variable() {
    let src = r#"module M {
    in  s : u8
    out y : u8
    for i in 0..1 {
        let i = s
        y = i
    }
}
test "the body let shadows the loop variable" {
    let dut = M { };
    dut.s = 42;
    step(1);
    assert_eq(dut.y, 42);
}
"#;
    sim("f_let", src, 1);
}

/// Denetim 20: gölgeleyen `let i` indekste: `xs[i]` = `xs[s]`; s = 2 →
/// 30. Hatada `xs[0]` = 10.
#[test]
fn a_body_let_that_shadows_the_loop_variable_is_the_index() {
    let src = format!(
        r#"module M {{
{XS_PORTS}    in  s : u2
    out y : u8
{XS_WIRE}    for i in 0..1 {{
        let i = s
        y = xs[i]
    }}
}}
test "the index is the local" {{
    let dut = M {{ }};
{XS_DRIVE}    dut.s = 2;
    step(1);
    assert_eq(dut.y, 30);
}}
"#
    );
    sim("f_index", &src, 1);
}

/// Denetim 21: gölgeleyen `let i` parça seçiminin başlangıcı: a = 182,
/// s = 1 → (182 >> 1) & 3 = 3. Hatada başlangıç 0: 182 & 3 = 2.
#[test]
fn a_body_let_that_shadows_the_loop_variable_is_the_part_select_start() {
    let src = r#"module M {
    in  a : u8
    in  s : u3
    out y : u2
    for i in 0..1 {
        let i = s
        y = a[i +: 2] as u2
    }
}
test "the start is the local" {
    let dut = M { };
    dut.a = 182;
    dut.s = 1;
    step(1);
    assert_eq(dut.y, 3);
}
"#;
    sim("f_part", src, 1);
}

/// Denetim 24: iç içe modül düzeyi `for`, aynı ad: iç `i` dıştakini
/// gölgeler; `ys[i] = xs[i]` iç döngünün dört değeriyle: ys = xs. Hatada
/// dört yazım da dış değerle `ys[0]`'a (E4001, E4012).
#[test]
fn an_inner_module_level_loop_variable_shadows_the_outer_one() {
    let src = format!(
        r#"module M {{
{XS_PORTS}{YS_PORTS}{XS_WIRE}{YS_WIRE}    for i in 0..1 {{
        for i in 0..4 {{ ys[i] = xs[i] }}
    }}
{YS_OUT}}}
test "the inner variable indexes" {{
    let dut = M {{ }};
{XS_DRIVE}    step(1);
{}}}
"#,
        asserts4([10, 20, 30, 40])
    );
    sim("f_nested_same", &src, 1);
}

/// Denetim 25–27: döngü değişkeni kök const'u, portu ve önceki modül
/// `let`'ini gölgeler (W1002); döngü içinde değişkendir, dışarıda dış
/// tanım. `for I`, `for i`, `for k` her biri `ys = xs`'i bir parça sürer;
/// `z = i` (port) = 9, `w = k` (modül let'i, s) = 5, `v = I` (const) = 1.
#[test]
fn a_module_level_loop_variable_shadows_a_const_a_port_and_a_module_let() {
    let src = format!(
        r#"const I : u32 = 1
module M {{
{XS_PORTS}    in  i : u8
    in  s : u8
{YS_PORTS}    out z : u8
    out w : u8
    out v : u32
{XS_WIRE}{YS_WIRE}    z = i
    let k = s
    w = k
    v = I
    for I in 0..2 {{ ys[I] = xs[I] }}
    for i in 2..3 {{ ys[i] = xs[i] }}
    for k in 3..4 {{ ys[k] = xs[k] }}
{YS_OUT}}}
test "inside the loop the variable, outside the outer definition" {{
    let dut = M {{ }};
{XS_DRIVE}    dut.i = 9;
    dut.s = 5;
    step(1);
{}    assert_eq(dut.z, 9);
    assert_eq(dut.w, 5);
    assert_eq(dut.v, 1);
}}
"#,
        asserts4([10, 20, 30, 40])
    );
    sim("f_outer", &src, 1);
}

/// Denetim 28: gövde `let C`'si kök const'u gölgeler: y = s = 42, const'un
/// 5'i değil.
#[test]
fn a_body_let_that_shadows_a_root_const_is_the_local() {
    let src = r#"const C : u8 = 5
module M {
    in  s : u8
    out y : u8
    for i in 0..1 {
        let C = s
        y = C
    }
}
test "the body let wins" {
    let dut = M { };
    dut.s = 42;
    step(1);
    assert_eq(dut.y, 42);
}
"#;
    sim("f_let_const", src, 1);
}

/// Denetim 30–32 (bekçi): açılan fn'in parametresi, `let`'i ve kök
/// const'u döngü değişkenine bağlanmaz. `f(i) = i ^ 1`, `g(a) = { let i
/// = a ^ 2; i }`, `pick(x) = x[I]` (const I = 1) döngü `for I` / `for i`
/// içinde: s = 40 → y = 41, z = 42; her k için ys[k] = xs[1] = 20.
#[test]
fn function_scopes_ignore_the_callers_module_level_loop_variable() {
    let src = format!(
        r#"const I : u32 = 1
fn f(i: u8) -> u8 {{ i ^ 1 }}
fn g(a: u8) -> u8 {{
    let i = a ^ 2
    i
}}
fn pick(x: [u8; 4]) -> u8 {{ x[I] }}
module M {{
    in  s : u8
{XS_PORTS}{YS_PORTS}    out y : u8
    out z : u8
{XS_WIRE}{YS_WIRE}    for i in 0..1 {{
        y = f(s)
        z = g(s)
    }}
    for I in 0..4 {{ ys[I] = pick(xs) }}
{YS_OUT}}}
test "the function reads its own names" {{
    let dut = M {{ }};
    dut.s = 40;
{XS_DRIVE}    step(1);
    assert_eq(dut.y, 41);
    assert_eq(dut.z, 42);
{}}}
"#,
        asserts4([20, 20, 20, 20])
    );
    sim("f_fn", &src, 1);
}

/// Denetim 33: kısaltılmış port bağlaması `P { i }` ≡ `P { i: i }`;
/// değer döngü değişkenidir: ys[k] = k: y0 = 0, y1 = 1. Hatada E1001.
#[test]
fn a_shorthand_port_binding_reads_the_loop_variable() {
    let src = r#"module P {
    in  i : u8
    out y : u8
    y = i
}
module M {
    out y0 : u8
    out y1 : u8
    wire ys : [u8; 2]
    for i in 0..2 {
        let p = P { i }
        ys[i] = p.y
    }
    y0 = ys[0]
    y1 = ys[1]
}
test "each instance gets its iteration value" {
    let dut = M { };
    step(1);
    assert_eq(dut.y0, 0);
    assert_eq(dut.y1, 1);
}
"#;
    sim("f_shorthand", src, 1);
}

/// Denetim 35–36: iç döngünün gövdesi. İç `let a` dış gövdenin `a`'sını
/// gölgeler: z = s (dış), y = t (iç). İç gövdenin `let i`'si dış döngü
/// değişkenini gölgeler: x = s. s = 4, t = 9 → z = 4, y = 9, x = 4.
/// Hatada x = 0 (dış döngünün değeri).
#[test]
fn an_inner_loop_body_let_shadows_the_outer_body_and_the_outer_variable() {
    let src = r#"module M {
    in  s : u8
    in  t : u8
    out x : u8
    out y : u8
    out z : u8
    for i in 0..1 {
        let a = s
        z = a
        for j in 0..1 {
            let a = t
            y = a
            let i = s
            x = i
        }
    }
}
test "the inner declarations win" {
    let dut = M { };
    dut.s = 4;
    dut.t = 9;
    step(1);
    assert_eq(dut.z, 4);
    assert_eq(dut.y, 9);
    assert_eq(dut.x, 4);
}
"#;
    sim("f_inner_body", src, 1);
}

/// Denetim 40–41 (bekçi): döngü değişkeni generic argüman (`W<i>`: 0, 1)
/// ve iki farklı adlı iç içe döngü (`ys[i * 2 + j] = xs[j * 2 + i]`:
/// ys = xs0, xs2, xs1, xs3 = 10, 30, 20, 40).
#[test]
fn loop_variables_as_generic_arguments_and_in_nested_indices() {
    let src = format!(
        r#"{W_MODULE}module M {{
{XS_PORTS}{YS_PORTS}    out z0 : u8
    out z1 : u8
{XS_WIRE}{YS_WIRE}    wire zs : [u8; 2]
    for i in 0..2 {{
        let w = W<i> {{ }}
        zs[i] = w.y
    }}
    for i in 0..2 {{
        for j in 0..2 {{ ys[i * 2 + j] = xs[j * 2 + i] }}
    }}
    z0 = zs[0]
    z1 = zs[1]
{YS_OUT}}}
test "values and transposition" {{
    let dut = M {{ }};
{XS_DRIVE}    step(1);
    assert_eq(dut.z0, 0);
    assert_eq(dut.z1, 1);
{}}}
"#,
        asserts4([10, 30, 20, 40])
    );
    sim("f_controls", &src, 1);
}

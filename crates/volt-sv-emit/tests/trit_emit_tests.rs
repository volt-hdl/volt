//! Trit SV eşlemesi (ADR-0003: 2 bit işaretli depolama).
//!
//! Kodlama ikiye tümleyen i2: +1 = 2'sb01, 0 = 2'sb00, -1 = 2'sb11.
//! `Trit * x` çarpan üretmez: +1 → x, -1 → -x, 0 → 0 seçicisi.

mod typed;

use volt_span::FileId;
use volt_sv_emit::{EmitResult, SvaMode};

/// Sürücünün yolu (`tests/typed`): Trit aritmetiğinin genişliği tip
/// denetiminin tablosundan okunur (#463, ADR-0145); tablosuz `emit` onda
/// E2005 verir.
fn compile(src: &str) -> EmitResult {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    let out = typed::emit_full(&parsed.ast, "test.volt", src, SvaMode::None);
    EmitResult {
        sv: out.sv,
        diagnostics: out.diagnostics,
    }
}

fn sv(src: &str) -> String {
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "emit hatasız olmalı: {:?}\n{}",
        result
            .diagnostics
            .iter()
            .map(|d| format!("{} {}", d.code.as_str(), d.message))
            .collect::<Vec<_>>(),
        result.sv
    );
    result.sv
}

/// `assign` satırları — başlıktaki yorumlar çarpan denetimini bozmasın.
fn assigns(out: &str) -> Vec<&str> {
    out.lines()
        .map(str::trim)
        .filter(|l| l.starts_with("assign ") || l.starts_with("wire "))
        .collect()
}

const MAC: &str = "module M { in w : Trit in x : i8 out y : i8 y = w * x }";

#[test]
fn trit_port_maps_to_signed_2bit() {
    let out = sv(MAC);
    assert!(out.contains("input  logic signed [1:0] w"), "çıktı:\n{out}");
}

#[test]
fn trit_times_int_is_a_mux_not_a_multiplier() {
    let out = sv(MAC);
    assert!(
        out.contains("assign y = w == 2'sb01 ? x : (w == 2'sb11 ? -(x) : 8'sd0);"),
        "çıktı:\n{out}"
    );
    assert!(
        assigns(&out).iter().all(|l| !l.contains('*')),
        "çıktı:\n{out}"
    );
}

#[test]
fn int_times_trit_is_symmetric() {
    let out = sv("module M { in w : Trit in x : i8 out y : i8 y = x * w }");
    assert!(
        out.contains("assign y = w == 2'sb01 ? x : (w == 2'sb11 ? -(x) : 8'sd0);"),
        "çıktı:\n{out}"
    );
}

#[test]
fn trit_mac_widens_operand_to_target() {
    // ADR-0041: hedef i16 → x açıkça genişletilir, sıfır da 16 bit.
    let out = sv("module M { in w : Trit in x : i8 out y : i16 let p : i16 = w * x y = p }");
    assert!(
        out.contains("w == 2'sb01 ? 16'(x) : (w == 2'sb11 ? -(16'(x)) : 16'sd0)"),
        "çıktı:\n{out}"
    );
    assert!(
        assigns(&out).iter().all(|l| !l.contains('*')),
        "çıktı:\n{out}"
    );
}

#[test]
fn trit_times_trit_is_a_mux() {
    let out = sv("module M { in a : Trit in b : Trit out y : Trit y = a * b }");
    assert!(
        out.contains("assign y = a == 2'sb01 ? b : (a == 2'sb11 ? -(b) : 2'sd0);"),
        "çıktı:\n{out}"
    );
}

#[test]
fn trit_sum_is_three_bits() {
    // Trit + Trit → i3 (type-inference.md §3.3); +1 + +1 = +2 taşmamalı.
    let out = sv("module M { in a : Trit in b : Trit out y : bool y = a + b == 2 }");
    assert!(out.contains("3'(a) + 3'(b) == 3'sd2"), "çıktı:\n{out}");
}

#[test]
fn trit_literals_and_negation() {
    let out =
        sv("module M { in a : Trit out y : Trit out z : Trit let n : Trit = -1 y = n z = -a }");
    assert!(
        out.contains("wire signed [1:0] n = -2'sd1;"),
        "çıktı:\n{out}"
    );
    assert!(out.contains("assign z = -a;"), "çıktı:\n{out}");
}

#[test]
fn trit_register_array_and_cast() {
    let out = sv("module M { in clk : clock in x : i8 out y : i8 \
         reg ws : [Trit; 4] = [0; 4] \
         reg acc : i8 = 0 \
         on clk { acc <= ws[1] * x } \
         y = acc + (ws[0] as i8) }");
    assert!(
        out.contains("logic signed [1:0] ws [0:3];"),
        "çıktı:\n{out}"
    );
    assert!(
        out.contains("ws[1] == 2'sb01 ? x : (ws[1] == 2'sb11 ? -(x) : 8'sd0)"),
        "çıktı:\n{out}"
    );
    assert!(out.contains("8'(ws[0])"), "çıktı:\n{out}");
}

#[test]
fn trit_no_longer_e0003() {
    let codes: Vec<_> = compile(MAC)
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert!(codes.is_empty(), "{codes:?}");
}

#[test]
fn unsupported_port_type_is_reported_once() {
    // Port tipi hem sembol geçişinde hem bildirimde sorgulanır; tanı bir
    // kez basılmalı (çift E0003 kök nedeni). ADR-0065: giriş yönlü reset
    // (ham port) artık desteklenir; çıkış yönlüsü kapsam dışı kalır.
    let src = "module M { out r : reset out y : bool y = true }";
    let codes: Vec<_> = compile(src)
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, vec!["E0003"], "{codes:?}");
}

#[test]
fn local_name_shadowing_trit_const_is_not_trit() {
    // Modül sembolü üst düzey const'u gölgeler (width_of ile aynı sıra):
    // i8 port K ile çarpım gerçek çarpımdır, seçici DEĞİL.
    let out = sv("const K : Trit = 1\nmodule M { in K : i8 in x : i8 out y : i8 y = K * x }");
    assert!(out.contains("assign y = K * x;"), "çıktı:\n{out}");
}

#[test]
fn trit_const_is_a_mux() {
    let out = sv("const K : Trit = -1\nmodule M { in x : i8 out y : i8 y = K * x }");
    assert!(!out.contains(" * "), "çıktı:\n{out}");
}

/// Tablosuz giriş noktası (`emit`): Trit aritmetiğinin genişliği tip
/// denetiminden gelir (#463, ADR-0145), üretici onu tahmin etmez; çarpım
/// E2005 alır, yeri çarpımın kendisidir.
#[test]
fn without_the_table_a_trit_product_is_e2005() {
    let parsed = volt_syntax::parser::parse(FileId(0), MAC);
    let out = volt_sv_emit::emit(&parsed.ast, "test.volt");
    let e: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E2005")
        .collect();
    assert_eq!(e.len(), 1, "{:?}", out.diagnostics);
    assert_eq!(
        e[0].message,
        "cannot determine the width of this Trit operation"
    );
    let span = e[0].primary_span().expect("konum").span;
    assert_eq!(&MAC[span.start as usize..span.end as usize], "w * x");
}

/// Karşılaştırma ve Trit'in kendisi Trit aritmetiği değildir: tablosuz da
/// üretilir (bool ve Trit'in bildirilen tipi).
#[test]
fn without_the_table_a_trit_comparison_still_emits() {
    let src = "module M { in a : Trit in b : Trit out y : bool out z : Trit y = a == b z = a }";
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    let out = volt_sv_emit::emit(&parsed.ast, "test.volt");
    assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
}

/// ADR-0145 Karar 8: `Trit × iN` bir işlemin işleneniyken iN'de sarar. İtilen
/// `+`'nın işleneni olan çarpım kendi genişliğinde (i13) hesaplanır ve
/// `14'($signed(...))` ile yalıtılır; tip denetimi o işlenene hedef tipi
/// (i14) kaydetse de genişlik iN yanından okunur.
#[test]
fn a_trit_product_under_a_pushed_sum_is_isolated_at_its_own_width() {
    let out = sv("module M { in t : Trit in x : i13 in y : i13 out o : i14 o = t * x + y }");
    let want = "assign o = 14'($signed(t == 2'sb01 ? x : (t == 2'sb11 ? -(x) : 13'sd0))) + 14'(y);";
    assert!(assigns(&out).contains(&want), "çıktı:\n{out}");
}

/// İki yanı aynı genişlikte olan i2 çarpımı da (Trit 2 bit, x : i2)
/// dönüşümün işleneniyken i2'de yalıtılır: t = -1, x = -2 → -2.
#[test]
fn an_i2_trit_product_under_a_cast_is_isolated_at_its_own_width() {
    let out = sv("module M { in t : Trit in x : i2 out o : i12 o = (t * x) as i12 }");
    let want = "assign o = 12'($signed((t == 2'sb01 ? x : (t == 2'sb11 ? -(x) : 2'sd0))));";
    assert!(assigns(&out).contains(&want), "çıktı:\n{out}");
}

/// Sınırın öteki yanı: en üstte geniş hedefe yazılan çarpım ADR-0041 ile
/// hedefe itilir (ADR-0062 Karar 2); x ve sıfır hedef genişliğinde.
#[test]
fn a_trit_product_at_the_top_of_a_wider_target_is_pushed() {
    let out = sv("module M { in t : Trit in x : i8 out o : i12 o = t * x }");
    let want = "assign o = t == 2'sb01 ? 12'(x) : (t == 2'sb11 ? -(12'(x)) : 12'sd0);";
    assert!(assigns(&out).contains(&want), "çıktı:\n{out}");
}

//! Trit SV eşlemesi (ADR-0003: 2 bit işaretli depolama).
//!
//! Kodlama ikiye tümleyen i2: +1 = 2'sb01, 0 = 2'sb00, -1 = 2'sb11.
//! `Trit * x` çarpan üretmez: +1 → x, -1 → -x, 0 → 0 seçicisi.

use volt_span::FileId;
use volt_sv_emit::{emit, EmitResult};

fn compile(src: &str) -> EmitResult {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    emit(&parsed.ast, "test.volt")
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
    // kez basılmalı (çift E0003 kök nedeni).
    let src = "module M { in r : reset out y : bool y = true }";
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

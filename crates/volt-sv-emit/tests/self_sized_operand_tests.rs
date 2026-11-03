//! Kendi genişliğinde hesaplanan alt ifade daha geniş bir bağlamda
//! (ADR-0106 §6). SystemVerilog'da boyut dönüşümü içini hedef genişlikte
//! hesaplar: `12'(a << 4)` (a = 8'hFF) `ff0` verir, Volt'ta `a << 4`
//! u8'dir ve `0f0` olmalıdır (Verilator 5.052'de ölçüldü). Kaydırma ve
//! `~` `W'($unsigned(...))` / `W'($signed(...))` ile yalıtılır; seçim,
//! dönüşüm ve birleştirme `W'(...)` ile açıkça genişletilir (Verilator
//! `-Wall` WIDTHEXPAND vermez).

use volt_span::FileId;
use volt_sv_emit::emit;

fn sv(body: &str) -> String {
    let src = format!(
        "module M {{\n    in  a : u8\n    in  b : u8\n    in  n : u3\n    in  x : i8\n    out y : u12\n    out z : i12\n\n{body}\n}}\n"
    );
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    emit(&parsed.ast, "test.volt").sv
}

#[test]
fn shift_under_wider_arithmetic_is_isolated() {
    let out = sv("    let w : u12 = (a << n) + b\n    y = w\n    z = 0");
    assert!(out.contains("12'($unsigned((a << n))) + 12'(b)"), "{out}");
}

#[test]
fn shift_assigned_to_a_wider_target_is_isolated() {
    let out = sv("    y = a << n\n    z = 0");
    assert!(out.contains("assign y = 12'($unsigned(a << n));"), "{out}");
}

#[test]
fn signed_shift_is_isolated_with_signed() {
    let out = sv("    let w : i12 = (x << n) + 1\n    z = w\n    y = 0");
    assert!(out.contains("12'($signed((x << n)))"), "{out}");
}

#[test]
fn bit_not_inverts_its_own_width() {
    let out = sv("    let w : u12 = (~a) + b\n    y = w\n    z = 0");
    assert!(out.contains("12'($unsigned(~a)) + 12'(b)"), "{out}");
    assert!(!out.contains("~(12'(a))"), "{out}");
}

#[test]
fn select_and_cast_are_widened_explicitly() {
    let out = sv("    let w : u12 = (a[7:4] as u4) + b[3:0] as u4\n    y = w\n    z = 0");
    assert!(out.contains("12'(a[7:4]) + 12'(b[3:0])"), "{out}");
}

#[test]
fn shift_inside_a_wrapped_comparison_side_is_isolated() {
    let src = "module M {\n    in  q : u32\n    in  k : u5\n    out o : bool\n\n    o = q == ((1u32 << k) - 1)[31:0] as u32\n}\n";
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    let out = emit(&parsed.ast, "test.volt").sv;
    assert!(
        out.contains("assign o = q == 32'(33'($unsigned((32'd1 << k))) - 33'd1);"),
        "{out}"
    );
}

//! Karşılaştırmanın genişliği ve bileşik değerden seçim (ADR-0106).
//!
//! - Tip denetiminin kabul ettiği karşılaştırma iki yanın doğal
//!   genişliğinde yapılır: `a + b == 5` (u8) taşma bitini atmaz,
//!   `a + b < 300` literali 8 bite kesmez (`8'd300` = 44 idi).
//! - Bileşik değerden bit/aralık seçimi boyut dönüşümüne iner:
//!   `(a + 8'd1)[3:0]` SystemVerilog sözdizimi hatasıdır.
//! - Taşma biti okunan tipsiz `let` için "unused" yorumu basılmaz.

use volt_span::FileId;
use volt_sv_emit::emit;

fn sv(src: &str) -> String {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    emit(&parsed.ast, "test.volt").sv
}

const PORTS: &str = "    in  a : u8\n    in  b : u8\n    in  c : u8\n    out o : bool\n";

#[test]
fn comparison_with_a_literal_keeps_the_carry() {
    let out = sv(&format!("module M {{\n{PORTS}\n    o = a + b == 5\n}}\n"));
    assert!(out.contains("assign o = 9'(a) + 9'(b) == 9'd5;"), "{out}");
}

#[test]
fn literal_wider_than_the_operands_is_not_truncated() {
    let out = sv(&format!("module M {{\n{PORTS}\n    o = a + b < 300\n}}\n"));
    assert!(out.contains("9'd300"), "{out}");
    assert!(!out.contains("8'd300"), "{out}");
}

#[test]
fn equal_natural_widths_compare_at_that_width() {
    let out = sv(&format!(
        "module M {{\n{PORTS}\n    o = a + b == c + a\n}}\n"
    ));
    assert!(
        out.contains("assign o = 9'(a) + 9'(b) == 9'(c) + 9'(a);"),
        "{out}"
    );
}

#[test]
fn constant_bound_keeps_the_narrow_width() {
    let out = sv(&format!(
        "const LIMIT : u8 = 6\nmodule M {{\n{PORTS}\n    o = c == LIMIT - 1\n}}\n"
    ));
    assert!(out.contains("assign o = c == 8'd6 - 8'd1;"), "{out}");
}

#[test]
fn range_of_a_computed_value_is_a_size_cast() {
    let out = sv("module M {\n    in  a : u8\n    in  b : u8\n    out y : u4\n    out t : bool\n    out h : u4\n\n    y = (a + 1)[3:0] as u4\n    t = (a + b)[8]\n    h = (a + b)[8:5] as u4\n}\n");
    assert!(out.contains("assign y = 4'(9'(a) + 9'd1);"), "{out}");
    assert!(
        out.contains("assign t = 1'((9'(a) + 9'(b)) >> 8);"),
        "{out}"
    );
    assert!(
        out.contains("assign h = 4'((9'(a) + 9'(b)) >> 5);"),
        "{out}"
    );
    assert!(!out.contains(")["), "SV bir ifadeyi seçemez: {out}");
}

#[test]
fn range_of_a_signal_is_unchanged() {
    let out = sv("module M {\n    in  a : u8\n    out y : u4\n\n    y = a[3:0] as u4\n}\n");
    assert!(out.contains("assign y = a[3:0];"), "{out}");
}

#[test]
fn carry_read_by_a_comparison_is_not_called_unused() {
    // Sol yan 8 bitlik işlemle başlıyordu: okuma genişliği 8 kaydedilir,
    // `s` ise 9 bit basılırdı; yorum yanlıştı.
    let out = sv(&format!(
        "module M {{\n{PORTS}\n    let s = c + a\n    o = a + b == s\n}}\n"
    ));
    assert!(out.contains("assign o = 9'(a) + 9'(b) == s;"), "{out}");
    assert!(!out.contains("carry bit of 's' unused"), "{out}");
}

#[test]
fn carry_dropped_by_an_assignment_is_still_reported() {
    let out = sv("module M {\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    let s = a + b\n    y = s\n}\n");
    assert!(out.contains("carry bit of 's' unused"), "{out}");
}

//! Bileşik değerden indexed part-select (#112, ADR-0106 §4 ile aynı
//! yöntem). SystemVerilog bir ifadeyi seçemez: `(a + b)[4 +: 4]`
//! Verilator 5.052'de sözdizimi hatasıdır. Değer doğal genişliğinde
//! hesaplanır (`a + b` u8 + u8 → 9 bit), seçimin alt ucu kadar kaydırılır
//! ve boyut dönüşümüyle kesilir; taşma biti (bit 8) seçilebilir.

use volt_span::FileId;
use volt_sv_emit::emit;

fn sv(body: &str) -> String {
    let src = format!(
        "module M {{\n    in  a : u8\n    in  b : u8\n    in  i : u4\n    in  x : i8\n    out y : bits<4>\n\n{body}\n}}\n"
    );
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    emit(&parsed.ast, "test.volt").sv
}

#[test]
fn ascending_with_a_constant_start() {
    let out = sv("    y = (a + b)[4 +: 4]");
    assert!(
        out.contains("assign y = 4'((9'(a) + 9'(b)) >> 4);"),
        "{out}"
    );
}

#[test]
fn ascending_with_a_variable_start() {
    let out = sv("    y = (a + b)[i +: 4]");
    assert!(
        out.contains("assign y = 4'((9'(a) + 9'(b)) >> i);"),
        "{out}"
    );
}

#[test]
fn descending_with_a_constant_start_reads_the_carry() {
    let out = sv("    y = (a + b)[8 -: 4]");
    assert!(
        out.contains("assign y = 4'((9'(a) + 9'(b)) >> 5);"),
        "{out}"
    );
}

#[test]
fn descending_with_a_variable_start() {
    let out = sv("    y = (a + b)[i -: 4]");
    assert!(
        out.contains("assign y = 4'((9'(a) + 9'(b)) >> (i - 4'd3));"),
        "{out}"
    );
}

#[test]
fn signed_value_is_selected_as_raw_bits() {
    let out = sv("    y = (x + 1)[2 +: 4]");
    assert!(
        out.contains("assign y = $unsigned(4'((9'(x) + 9'sd1) >> 2));"),
        "{out}"
    );
}

#[test]
fn part_select_of_a_signal_is_unchanged() {
    let out = sv("    y = a[i +: 4]");
    // #123: indeks tabanın adres genişliğinde (a : u8 → 3 bit; i : u4
    // olduğu gibi Verilator WIDTHTRUNC verirdi).
    assert!(out.contains("assign y = a[3'(i) +: 4];"), "{out}");
    assert!(!out.contains(")["), "{out}");
}

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
    // #123: i : u4, a : u8'in adres genişliği (3 bit) değil; `a[i +: 4]`
    // Verilator'da WIDTHTRUNC (ölümcül) verirdi, kaydırmaya iner.
    assert!(out.contains("assign y = 4'((a) >> i);"), "{out}");
    assert!(!out.contains(")["), "{out}");
}

/// Sabit indeks seçilebilir tabanda seçim biçiminde kalır; indeksin tipi
/// vektörün adres genişliğinden (u8 → 3 bit) farklı olsa da: genişlik
/// uyuşmazlığı (#123) yalnız değişken indeksin sorunudur. `K : u4` (4 bit,
/// 32 değil) kaydırmaya inmez: `a[4'd2]`, `a[4'd2 +: 4]` (const bildirilen
/// tipinde boyutlu literal, ADR-0041).
#[test]
fn a_constant_index_of_another_width_keeps_the_select() {
    let src = "const K : u4 = 2
module M {
    in  a : u8
    out o : bool
    out y : bits<4>
    o = a[K]
    y = a[K +: 4]
}
";
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    let out = emit(&parsed.ast, "test.volt").sv;
    assert!(out.contains("assign o = a[4'd2];"), "{out}");
    assert!(out.contains("assign y = a[4'd2 +: 4];"), "{out}");
}

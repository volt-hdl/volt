//! `as` ve literallerin SV işareti (fark testi bulguları, ADR-0108).
//!
//! #115: genişlik değiştiren dönüşümün SV ifadesi hedefin işaretini
//! taşımalı. Birleştirme (`{{N{x[msb]}}, x}`) ve dilim (`x[W-1:0]`) SV'de
//! işaretsizdir, boyut dönüşümü (`W'(e)`) kaynağın işaretini korur; işaretli
//! hedefte `$signed`, işaretsiz hedefte `$unsigned` sınırı gerekir, yoksa
//! karşılaştırma işaretsiz, `>>` mantıksal, dış genişletme yanlış olur.
//!
//! #116: işaretli bağlamda onaltılık/ikili literal işaretli (`'sh`); bir
//! işaretsiz işlenen SV'de bütün işlemi işaretsiz yapar. En küçük negatif
//! literal örüntüsüyle (`1'sh1`): `-1'sd1` geniş bağlamda +1 olurdu.

use volt_span::FileId;
use volt_sv_emit::emit;

fn sv(ports: &str, body: &str) -> String {
    let src = format!("module M {{\n{ports}\n{body}\n}}\n");
    let parsed = volt_syntax::parser::parse(FileId(0), &src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    emit(&parsed.ast, "test.volt").sv
}

#[test]
fn sign_extension_to_a_signed_type_is_signed() {
    let out = sv(
        "    in  a : i10\n    out o : bool",
        "    o = a as i11 > 880",
    );
    assert!(
        out.contains("assign o = $signed({{1{a[9]}}, a}) > 11'sd880;"),
        "{out}"
    );
}

#[test]
fn zero_extension_to_a_signed_type_shifts_arithmetically() {
    let out = sv(
        "    in  a : u22\n    in  n : u1\n    out o : i38",
        "    o = a as i38 >> n",
    );
    assert!(
        out.contains("assign o = $signed({{16{1'b0}}, a}) >>> n;"),
        "{out}"
    );
}

#[test]
fn narrowing_to_a_signed_type_is_signed() {
    let out = sv("    in  a : i36\n    out o : bool", "    o = a as i1 > 0");
    assert!(out.contains("assign o = $signed(a[0:0]) > 1'sd0;"), "{out}");
}

#[test]
fn narrowing_a_signed_value_to_an_unsigned_type_is_unsigned() {
    let out = sv(
        "    in  a : i14\n    in  b : i14\n    out o : u10",
        "    o = (a + b) as u7",
    );
    assert!(
        out.contains("assign o = 10'($unsigned(7'((a + b))));"),
        "{out}"
    );
}

#[test]
fn negating_a_sign_extended_value() {
    let out = sv("    in  a : i2\n    out o : i5", "    o = -(a as i4)");
    assert!(
        out.contains("assign o = -(5'($signed({{2{a[1]}}, a})));"),
        "{out}"
    );
}

#[test]
fn hex_literal_in_a_signed_context_is_signed() {
    let out = sv(
        "    in  a : i50\n    in  b : i18\n    out o : bool\n    out p : bool",
        "    o = a > 0x0\n    p = (b | 0x1FFFF) > 95146",
    );
    assert!(out.contains("assign o = a > 50'sh0;"), "{out}");
    assert!(
        out.contains("assign p = (b | 18'sh1FFFF) > 18'sd95146;"),
        "{out}"
    );
}

#[test]
fn hex_literal_in_an_unsigned_context_stays_unsigned() {
    let out = sv("    in  a : u8\n    out o : bool", "    o = a > 0x7F");
    assert!(out.contains("assign o = a > 8'h7F;"), "{out}");
}

#[test]
fn most_negative_literal_is_written_as_its_pattern() {
    let out = sv(
        "    in  a : i1\n    in  b : i8\n    out o : u35\n    out p : bool",
        "    o = (a * (-1)) as u35\n    p = b == (-128)",
    );
    assert!(
        out.contains("assign o = $unsigned(35'((a * 1'sh1)));"),
        "{out}"
    );
    assert!(out.contains("assign p = b == 8'sh80;"), "{out}");
    assert!(
        !out.contains("-1'sd1") && !out.contains("-8'sd128"),
        "{out}"
    );
}

//! Tip kontrolü (F2a) testleri — docs/spec/type-inference.md §0-§5, §11.
//!
//! Kapsam: tip gösterimi, çift yönlü kontrol (synth/check), literal
//! çözümleme, tekli operatörler, bit/aralık seçimi, cast, atanabilirlik
//! ve sürücü analizi (E4001/E4002/W4001/W4002). İkili operatörler F2b —
//! F2a'da sessizce Ty::Error döner, hata üretmez.

use volt_hir::{analyze, AnalysisResult};
use volt_syntax::{parse, FileId};

fn check(src: &str) -> AnalysisResult {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "kaynak ayrışmalı: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
}

fn codes(src: &str) -> Vec<&'static str> {
    check(src).error_codes()
}

/// İsme göre tanımın çıkarılan tipinin metin gösterimi.
fn def_ty(result: &AnalysisResult, name: &str) -> String {
    let (def, _) = result
        .resolve
        .def_by_name(name)
        .unwrap_or_else(|| panic!("'{name}' tanımı bulunmalı"));
    let id = *result
        .typeck
        .def_types
        .get(&def)
        .unwrap_or_else(|| panic!("'{name}' için tip kaydı bulunmalı"));
    result.typeck.types.display(id)
}

// ═══ Literaller (§3.1, §4) ════════════════════════════════════════

#[test]
fn suffixed_literal_synthesizes_concrete_type() {
    let result =
        check("module M {\n    in  a : u8\n    out y : u8\n\n    let _x = 42u8\n\n    y = a\n}\n");
    assert_eq!(def_ty(&result, "_x"), "u8");
    assert!(!result.error_codes().contains(&"W2012"));
}

#[test]
fn unsuffixed_literal_defaults_to_i32_with_w2012() {
    let result =
        check("module M {\n    in  a : u8\n    out y : u8\n\n    let _x = 42\n\n    y = a\n}\n");
    assert!(result.error_codes().contains(&"W2012"));
    assert_eq!(def_ty(&result, "_x"), "i32");
}

#[test]
fn literal_at_u8_max_fits() {
    let c = codes("module M {\n    in  clk : clock\n    out y : u8\n\n    reg r : u8 = 255\n\n    on clk { r <= r }\n    y = r\n}\n");
    assert!(!c.contains(&"E2010"), "{c:?}");
}

#[test]
fn literal_overflow_in_reg_init_e2010() {
    let c = codes("module M {\n    in  clk : clock\n    out y : u8\n\n    reg r : u8 = 300\n\n    on clk { r <= r }\n    y = r\n}\n");
    assert!(c.contains(&"E2010"), "{c:?}");
}

#[test]
fn suffixed_literal_overflow_e2010() {
    let c =
        codes("module M {\n    in  a : u8\n    out y : u8\n\n    let _x = 300u8\n\n    y = a\n}\n");
    assert!(c.contains(&"E2010"), "{c:?}");
}

#[test]
fn signed_literal_overflow_e2010() {
    let c = codes("module M {\n    in  clk : clock\n    out y : i8\n\n    reg r : i8 = 128\n\n    on clk { r <= r }\n    y = r\n}\n");
    assert!(c.contains(&"E2010"), "{c:?}");
}

#[test]
fn signed_literal_at_max_fits() {
    let c = codes("module M {\n    in  clk : clock\n    out y : i8\n\n    reg r : i8 = 127\n\n    on clk { r <= r }\n    y = r\n}\n");
    assert!(!c.contains(&"E2010"), "{c:?}");
}

#[test]
fn bool_literal_checks_against_bool() {
    let result = check("module M {\n    in  clk : clock\n    out y : bool\n\n    reg f : bool = true\n\n    on clk { f <= f }\n    y = f\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn numeric_literal_in_bool_context_e2003() {
    let c = codes("module M {\n    in  a : u8\n    out y : bool\n\n    y = 5\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn trit_literal_zero_or_one_ok() {
    let c = codes("module M {\n    in  clk : clock\n    out t : Trit\n\n    reg r : Trit = 1\n\n    on clk { r <= r }\n    t = r\n}\n");
    assert!(!c.contains(&"E2011"), "{c:?}");
}

#[test]
fn trit_literal_out_of_range_e2011() {
    let c = codes("module M {\n    in  clk : clock\n    out t : Trit\n\n    reg r : Trit = 2\n\n    on clk { r <= r }\n    t = r\n}\n");
    assert!(c.contains(&"E2011"), "{c:?}");
}

// ═══ Atanabilirlik (§5) ═══════════════════════════════════════════

#[test]
fn same_type_assign_ok() {
    let result = check("module M {\n    in  a : u8\n    out y : u8\n\n    y = a\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn implicit_narrowing_e2001() {
    let c = codes("module M {\n    in  a : u16\n    out y : u8\n\n    y = a\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn implicit_widening_e2001() {
    let c = codes("module M {\n    in  a : u8\n    out y : u16\n\n    y = a\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn sign_mismatch_assign_e2002() {
    let c = codes("module M {\n    in  a : u8\n    out y : i8\n\n    y = a\n}\n");
    assert!(c.contains(&"E2002"), "{c:?}");
}

#[test]
fn bits_width_mismatch_e2001() {
    let c = codes("module M {\n    in  a : bits<4>\n    out y : bits<8>\n\n    y = a\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn bits_to_uint_without_cast_e2003() {
    let c = codes("module M {\n    in  a : bits<8>\n    out y : u8\n\n    y = a\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn let_with_explicit_type_checks_value_e2001() {
    let c = codes(
        "module M {\n    in  a : u16\n    out y : u16\n\n    let _x : u8 = a\n\n    y = a\n}\n",
    );
    assert!(c.contains(&"E2001"), "{c:?}");
}

// ═══ Tekli operatörler (§3.4) ═════════════════════════════════════

#[test]
fn logical_not_on_bool_ok() {
    let result = check("module M {\n    in  f : bool\n    out y : bool\n\n    y = !f\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn logical_not_on_uint_e2003() {
    let c = codes("module M {\n    in  a : u8\n    out y : bool\n\n    y = !a\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn bitnot_preserves_width() {
    let result = check("module M {\n    in  a : u8\n    out y : u8\n\n    y = ~a\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn negation_widens_i8_to_i9() {
    let result = check("module M {\n    in  a : i8\n    out y : i9\n\n    y = -a\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn negation_result_does_not_fit_i8_e2001() {
    let c = codes("module M {\n    in  a : i8\n    out y : i8\n\n    y = -a\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn negation_on_unsigned_e2002() {
    let c = codes("module M {\n    in  a : u8\n    out y : i9\n\n    y = -a\n}\n");
    assert!(c.contains(&"E2002"), "{c:?}");
}

#[test]
fn negation_on_trit_stays_trit() {
    let result = check("module M {\n    in  w : Trit\n    out t : Trit\n\n    t = -w\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ Bit ve aralık seçimi (§3.5) ══════════════════════════════════

#[test]
fn bit_select_synthesizes_bool() {
    let result = check("module M {\n    in  d : u8\n    out y : bool\n\n    y = d[3]\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn bit_select_out_of_bounds_e2006() {
    let c = codes("module M {\n    in  d : u8\n    out y : bool\n\n    y = d[9]\n}\n");
    assert!(c.contains(&"E2006"), "{c:?}");
}

#[test]
fn variable_index_skips_bound_check() {
    let result =
        check("module M {\n    in  d : u8\n    in  n : u8\n    out y : bool\n\n    y = d[n]\n}\n");
    // Çalışma zamanı indeksi: E2006 da E2021 de üretilmemeli.
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn const_index_out_of_bounds_e2006() {
    let c = codes("const IDX : u32 = 9;\n\nmodule M {\n    in  d : u8\n    out y : bool\n\n    y = d[IDX]\n}\n");
    assert!(c.contains(&"E2006"), "{c:?}");
}

#[test]
fn range_select_synthesizes_bits() {
    let result = check("module M {\n    in  d : u8\n    out s : bits<4>\n\n    s = d[7:4]\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn reversed_range_e2007() {
    let c = codes("module M {\n    in  d : u8\n    out s : bits<4>\n\n    s = d[3:7]\n}\n");
    assert!(c.contains(&"E2007"), "{c:?}");
}

#[test]
fn range_out_of_bounds_e2006() {
    let c = codes("module M {\n    in  d : u8\n    out s : bits<4>\n\n    s = d[9:6]\n}\n");
    assert!(c.contains(&"E2006"), "{c:?}");
}

#[test]
fn variable_range_bound_e2008() {
    let c = codes(
        "module M {\n    in  d : u8\n    in  n : u8\n    out s : bits<4>\n\n    s = d[n:0]\n}\n",
    );
    assert!(c.contains(&"E2008"), "{c:?}");
}

#[test]
fn index_on_clock_e2003() {
    let c = codes("module M {\n    in  clk : clock\n    out y : bool\n\n    y = clk[0]\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

// ═══ Tip dönüşümü (§3.6) ══════════════════════════════════════════

#[test]
fn cast_widening_ok() {
    let result = check("module M {\n    in  a : u8\n    out y : u16\n\n    y = a as u16\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn cast_narrowing_warns_w2010() {
    let result = check("module M {\n    in  a : u16\n    out y : u8\n\n    y = a as u8\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert!(result.error_codes().contains(&"W2010"));
}

#[test]
fn cast_sign_change_ok() {
    let result = check("module M {\n    in  a : u8\n    out y : i8\n\n    y = a as i8\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn cast_bits_and_uint_roundtrip_ok() {
    let result = check("module M {\n    in  a : bits<8>\n    in  b : u8\n    out y : u8\n    out z : bits<8>\n\n    y = a as u8\n    z = b as bits<8>\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn cast_signed_to_trit_e2009() {
    let c = codes("module M {\n    in  v : i8\n    out t : Trit\n\n    t = v as Trit\n}\n");
    assert!(c.contains(&"E2009"), "{c:?}");
}

#[test]
fn cast_unsigned_to_trit_e2009() {
    let c = codes("module M {\n    in  v : u8\n    out t : Trit\n\n    t = v as Trit\n}\n");
    assert!(c.contains(&"E2009"), "{c:?}");
}

#[test]
fn cast_trit_to_i2_ok() {
    let result = check("module M {\n    in  w : Trit\n    out y : i2\n\n    y = w as i2\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn cast_trit_to_i1_e2009() {
    let c = codes("module M {\n    in  w : Trit\n    out y : i1\n\n    y = w as i1\n}\n");
    assert!(c.contains(&"E2009"), "{c:?}");
}

#[test]
fn cast_bool_u1_roundtrip_ok() {
    let result = check("module M {\n    in  f : bool\n    in  b : u1\n    out y : u1\n    out z : bool\n\n    y = f as u1\n    z = b as bool\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn cast_bool_to_u8_e2009() {
    let c = codes("module M {\n    in  f : bool\n    out y : u8\n\n    y = f as u8\n}\n");
    assert!(c.contains(&"E2009"), "{c:?}");
}

#[test]
fn cast_literal_to_u16_ok() {
    let result = check("module M {\n    in  a : u8\n    out y : u16\n\n    y = 5 as u16\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ Çift yönlü akış (§2, §4) ═════════════════════════════════════

#[test]
fn if_expr_pushes_expected_type_to_branches() {
    let result = check("module M {\n    in  s : bool\n    in  a : u8\n    out y : u8\n\n    y = if s { a } else { 0 }\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn if_cond_not_bool_e2003() {
    let c = codes("module M {\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    y = if a { b } else { b }\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn reg_without_type_and_literal_init_e2012() {
    let c = codes("module M {\n    in  clk : clock\n    out y : u8\n\n    reg r = 0\n\n    on clk { r <= r }\n    y = r\n}\n");
    assert!(c.contains(&"E2012"), "{c:?}");
}

#[test]
fn reg_without_type_infers_from_non_literal_init() {
    let result = check("module M {\n    in  clk : clock\n    in  a : u8\n    out y : u8\n\n    reg r = a\n\n    on clk { r <= a }\n    y = r\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "r"), "u8");
}

// ═══ F2b: ikili operatörler artık denetleniyor ════════════════════

#[test]
fn binary_arith_width_mismatch_e2001_in_f2b() {
    // fail/02 senaryosu — F2a'da sessizdi, F2b'de E2001.
    let c = codes("module M {\n    in  small : u8\n    in  large : u16\n    out sum   : u16\n\n    sum = small + large\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn comparison_of_mismatched_types_e2003_in_f2b() {
    let c = codes(
        "module M {\n    in  a : u8\n    in  b : u16\n    out y : bool\n\n    y = a == b\n}\n",
    );
    assert!(c.contains(&"E2003"), "{c:?}");
}

// ═══ Sürücü analizi (§11) ═════════════════════════════════════════

#[test]
fn double_driver_e4001() {
    let c = codes(
        "module M {\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    y = a\n    y = b\n}\n",
    );
    assert!(c.contains(&"E4001"), "{c:?}");
}

#[test]
fn conditional_assigns_in_same_on_block_single_driver() {
    let c = codes("module M {\n    in  clk : clock\n    in  s : bool\n    out q : u8\n\n    reg r : u8 = 0\n\n    on clk {\n        if s { r <= 1 } else { r <= 2 }\n    }\n\n    q = r\n}\n");
    assert!(!c.contains(&"E4001"), "{c:?}");
}

#[test]
fn two_on_blocks_driving_same_reg_e4001() {
    let c = codes("module M {\n    in  clk : clock\n    out q : u8\n\n    reg r : u8 = 0\n\n    on clk { r <= 1 }\n    on clk { r <= 2 }\n\n    q = r\n}\n");
    assert!(c.contains(&"E4001"), "{c:?}");
}

#[test]
fn e4001_carries_secondary_span_and_help() {
    let result = check(
        "module M {\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    y = a\n    y = b\n}\n",
    );
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E4001")
        .expect("E4001 bekleniyor");
    assert!(
        diag.spans.iter().any(|s| !s.primary),
        "ikincil konum olmalı"
    );
    assert!(!diag.help.as_deref().unwrap_or("").is_empty());
}

#[test]
fn undriven_output_e4002() {
    let c = codes("module M {\n    in  a : u8\n    out y : u8\n    out z : u8\n\n    y = a\n}\n");
    assert!(c.contains(&"E4002"), "{c:?}");
}

#[test]
fn driven_outputs_no_e4002() {
    let c = codes(
        "module M {\n    in  a : u8\n    out y : u8\n    out z : u8\n\n    y = a\n    z = a\n}\n",
    );
    assert!(!c.contains(&"E4002"), "{c:?}");
}

#[test]
fn output_driven_from_on_block_no_e4002() {
    let c = codes("module M {\n    in  clk : clock\n    in  d : u8\n    out q : u8\n\n    on clk { q <= d }\n}\n");
    assert!(!c.contains(&"E4002"), "{c:?}");
}

#[test]
fn partial_bit_assigns_no_e4001() {
    let c = codes("module M {\n    in  a : bool\n    in  b : bool\n    out y : bits<2>\n\n    y[0] = a\n    y[1] = b\n}\n");
    assert!(!c.contains(&"E4001"), "{c:?}");
    assert!(!c.contains(&"E4002"), "{c:?}");
}

#[test]
fn written_never_read_register_w4002() {
    let c = codes("module M {\n    in  clk : clock\n    in  d : u8\n    out y : u8\n\n    reg r : u8 = 0\n\n    on clk { r <= d }\n\n    y = d\n}\n");
    assert!(c.contains(&"W4002"), "{c:?}");
}

#[test]
fn driven_never_read_wire_w4001() {
    let c = codes("module M {\n    in  a : u8\n    out y : u8\n\n    wire t : u8\n\n    t = a\n    y = a\n}\n");
    assert!(c.contains(&"W4001"), "{c:?}");
}

#[test]
fn underscore_prefix_silences_w4001() {
    let c = codes("module M {\n    in  a : u8\n    out y : u8\n\n    wire _t : u8\n\n    _t = a\n    y = a\n}\n");
    assert!(!c.contains(&"W4001"), "{c:?}");
}

// ═══ Modül örnekleme ══════════════════════════════════════════════

#[test]
fn instance_output_port_field_type_flows() {
    let result = check("module Adder {\n    in  a : u8\n    out s : u9\n\n    s = a as u9\n}\n\nmodule Top {\n    in  x : u8\n    out y : u9\n\n    let add = Adder { a: x }\n\n    y = add.s\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn instance_input_binding_type_mismatch_e2001() {
    let c = codes("module Adder {\n    in  a : u8\n    out s : u9\n\n    s = a as u9\n}\n\nmodule Top {\n    in  x : u16\n    out y : u9\n\n    let add = Adder { a: x }\n\n    y = add.s\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

// ═══ Hata kaskadı bastırma ════════════════════════════════════════

#[test]
fn error_type_suppresses_cascade() {
    // Tanımsız isim E1001 üretir; ardından gelen atama Ty::Error ile
    // uyumlu sayılmalı — E2xxx kaskadı olmamalı.
    let parsed = parse(
        FileId(0),
        "module M {\n    in  a : u8\n    out y : u16\n\n    y = bilinmeyen\n}\n",
    );
    assert!(parsed.diagnostics.is_empty());
    let result = analyze(&parsed.ast);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| !d.code.is_warning())
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(errors, vec!["E1001"], "yalnız E1001 kalmalı: {errors:?}");
}

#[test]
fn cast_from_error_type_is_silent() {
    let parsed = parse(
        FileId(0),
        "module M {\n    in  a : u8\n    out y : u16\n\n    y = bilinmeyen as u16\n}\n",
    );
    assert!(parsed.diagnostics.is_empty());
    let result = analyze(&parsed.ast);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| !d.code.is_warning())
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(errors, vec!["E1001"], "{errors:?}");
}

// ═══ Dizi, demet ve bileşik tipler ════════════════════════════════

#[test]
fn array_wire_index_yields_element_type() {
    let result = check("module M {\n    in  a : bool\n    out y : bool\n\n    wire t : [bool; 4]\n\n    t[0] = a\n    y = t[1]\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "t"), "[bool; 4]");
}

#[test]
fn array_index_out_of_bounds_e2006() {
    let c = codes("module M {\n    in  a : bool\n    out y : bool\n\n    wire t : [bool; 4]\n\n    t[0] = a\n    y = t[9]\n}\n");
    assert!(c.contains(&"E2006"), "{c:?}");
}

#[test]
fn comb_block_assign_checked() {
    let result = check(
        "module M {\n    in  a : u8\n    out y : u8\n\n    comb {\n        y = a\n    }\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn comb_block_conditional_assigns_single_driver() {
    let c = codes("module M {\n    in  s : bool\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    comb {\n        if s { y = a } else { y = b }\n    }\n}\n");
    assert!(!c.contains(&"E4001"), "{c:?}");
}

#[test]
fn if_expr_synthesized_in_let() {
    let result = check("module M {\n    in  s : bool\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    let v = if s { a } else { b }\n\n    y = v\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "v"), "u8");
}

#[test]
fn if_expr_literal_then_adapts_to_concrete_else() {
    let result = check("module M {\n    in  s : bool\n    in  b : u8\n    out y : u8\n\n    let v = if s { 0 } else { b }\n\n    y = v\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "v"), "u8");
}

#[test]
fn if_expr_all_literal_branches_default_i32() {
    let result = check("module M {\n    in  s : bool\n    in  a : u8\n    out y : u8\n\n    let _v = if s { 0 } else { 1 }\n\n    y = a\n}\n");
    assert!(result.error_codes().contains(&"W2012"));
    assert_eq!(def_ty(&result, "_v"), "i32");
}

#[test]
fn match_stmt_in_on_block_checked() {
    let c = codes("module M {\n    in  clk : clock\n    in  s : u8\n    out q : u8\n\n    reg r : u8 = 0\n\n    on clk {\n        match s {\n            0 => { r <= 1 }\n            _ => { r <= 2 }\n        }\n    }\n\n    q = r\n}\n");
    assert!(!c.contains(&"E4001"), "{c:?}");
}

#[test]
fn cast_of_widened_arith_result_ok() {
    // İkili operatör sonucu (u9) açık dönüşümle genişletilebilir.
    let result = check("module M {\n    in  a : u8\n    in  b : u8\n    out y : u16\n\n    y = (a + b) as u16\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn negation_of_literal_stays_literal_and_checks() {
    let result = check("module M {\n    in  clk : clock\n    out y : i8\n\n    reg r : i8 = -1\n\n    on clk { r <= r }\n    y = r\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ İfade tipleri kaydı ══════════════════════════════════════════

#[test]
fn expression_types_are_recorded() {
    let result = check("module M {\n    in  a : u8\n    out y : u8\n\n    y = a\n}\n");
    assert!(
        !result.typeck.expr_types.is_empty(),
        "ifade tip haritası dolu olmalı"
    );
}

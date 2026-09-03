//! İkili operatör tip kuralları (F2b) — docs/spec/type-inference.md §3.3.
//!
//! Kapsam: aritmetik taşma genişlemesi ve esnek aralık (ADR-0025), Trit
//! kuralları, bit düzeyi operatörler, kaydırma (W2013), karşılaştırma,
//! mantıksal operatörler ve koşullu ifade dal birleşimi. §10 test
//! vektörleri burada birebir doğrulanır.

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

// ═══ §10 test vektörleri ══════════════════════════════════════════

#[test]
fn spec_vectors_arith_and_bitwise() {
    let result = check(
        "module M {\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    let _sum  = a + b\n    let _prod = a * b\n    let _and  = a & b\n\n    y = a\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "_sum"), "u9", "a + b → u9");
    assert_eq!(def_ty(&result, "_prod"), "u16", "a * b → u16");
    assert_eq!(def_ty(&result, "_and"), "u8", "a & b → u8");
}

#[test]
fn spec_vectors_trit() {
    let result = check(
        "module M {\n    in  t : Trit\n    in  x : i8\n    out y : i8\n\n    let _mul = t * t\n    let _add = t + t\n    let _mac = t * x\n\n    y = x\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "_mul"), "Trit", "t * t → Trit");
    assert_eq!(def_ty(&result, "_add"), "i3", "t + t → i3");
    assert_eq!(def_ty(&result, "_mac"), "i8", "t * x → i8");
}

#[test]
fn spec_vector_comparison_is_bool() {
    let result = check(
        "module M {\n    in  a : u8\n    in  b : u8\n    out y : bool\n\n    y = a == b\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

// ═══ Aritmetik: işaretsiz genişleme ═══════════════════════════════

#[test]
fn add_widens_into_exact_target() {
    let c =
        codes("module M {\n    in  a : u8\n    in  b : u8\n    out s : u9\n\n    s = a + b\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn sub_widens_one_bit() {
    let result = check(
        "module M {\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    let _d = a - b\n\n    y = a\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "_d"), "u9");
}

#[test]
fn mul_doubles_width_into_exact_target() {
    let c =
        codes("module M {\n    in  a : u8\n    in  b : u8\n    out p : u16\n\n    p = a * b\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn div_and_rem_do_not_widen() {
    let result = check(
        "module M {\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    let _q = a / b\n    let _r = a % b\n\n    y = a\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "_q"), "u8");
    assert_eq!(def_ty(&result, "_r"), "u8");
}

#[test]
fn div_result_does_not_stretch_to_wider_target_e2001() {
    let c =
        codes("module M {\n    in  a : u8\n    in  b : u8\n    out q : u9\n\n    q = a / b\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

/// ADR-0025: sayaç deseni — taşma biti hedefe göre atılır.
#[test]
fn counter_pattern_add_wraps_to_operand_width() {
    let c = codes("module M {\n    in  clk : clock\n    out y : u8\n\n    reg r : u8 = 0\n\n    on clk { r <= r + 1 }\n\n    y = r\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

/// ADR-0025: esnek aralık [8,9] dışına örtük çıkış yok.
#[test]
fn add_result_beyond_natural_width_e2001() {
    let c =
        codes("module M {\n    in  a : u8\n    in  b : u8\n    out s : u10\n\n    s = a + b\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn arith_operand_width_mismatch_e2001() {
    let c =
        codes("module M {\n    in  a : u8\n    in  b : u16\n    out s : u16\n\n    s = a + b\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn arith_width_mismatch_diag_has_help_and_reason() {
    let result =
        check("module M {\n    in  a : u8\n    in  b : u16\n    out s : u16\n\n    s = a + b\n}\n");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E2001")
        .expect("E2001 bekleniyor");
    assert!(diag.message.contains("u8") && diag.message.contains("u16"));
    assert!(!diag.help.as_deref().unwrap_or("").is_empty());
    assert!(!diag.notes.is_empty(), "neden notu taşımalı");
}

/// pass/04 aynası: çarpım sonucu esnek aralığıyla toplama operandı olur.
#[test]
fn nested_mul_result_adapts_as_add_operand() {
    let c = codes("module M {\n    in  a : u8\n    in  b : u8\n    in  c : u8\n    out r : u16\n\n    r = a as u16 + (b as u16) * (c as u16)\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

/// pass/06 aynası: esnek tip let üzerinden akıp hedef genişliğe uyar.
#[test]
fn flex_type_flows_through_let_binding() {
    let c = codes("module M {\n    in  a : u8\n    in  b : u8\n    in  c : u8\n    out y : u16\n\n    let sum = a + b\n    let widened = sum as u16\n    let scaled = widened * (c as u16)\n\n    y = scaled\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

// ═══ Aritmetik: işaretli ══════════════════════════════════════════

#[test]
fn signed_add_widens_one_bit() {
    let result = check(
        "module M {\n    in  a : i8\n    in  b : i8\n    out y : i8\n\n    let _s = a + b\n\n    y = a\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "_s"), "i9");
}

#[test]
fn signed_mul_doubles_width() {
    let result = check(
        "module M {\n    in  a : i8\n    in  b : i8\n    out y : i8\n\n    let _p = a * b\n\n    y = a\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "_p"), "i16");
}

#[test]
fn signed_div_keeps_width() {
    let result = check(
        "module M {\n    in  a : i8\n    in  b : i8\n    out y : i8\n\n    let _q = a / b\n\n    y = a\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "_q"), "i8");
}

#[test]
fn signed_accumulator_wraps_to_operand_width() {
    let c = codes("module M {\n    in  clk : clock\n    in  d : i16\n    out y : i16\n\n    reg acc : i16 = 0\n\n    on clk { acc <= acc + d }\n\n    y = acc\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn signed_width_mismatch_e2001() {
    let c =
        codes("module M {\n    in  a : i8\n    in  b : i16\n    out s : i16\n\n    s = a + b\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

// ═══ Aritmetik: işaret karışımı ve uyumsuz tipler ═════════════════

#[test]
fn mixed_sign_add_e2002() {
    let c = codes("module M {\n    in  a : u8\n    in  b : i8\n    out r : i16\n\n    r = (a + b) as i16\n}\n");
    assert!(c.contains(&"E2002"), "{c:?}");
}

#[test]
fn mixed_sign_mul_e2002() {
    let c = codes("module M {\n    in  a : u8\n    in  b : i8\n    out r : i16\n\n    r = (a * b) as i16\n}\n");
    assert!(c.contains(&"E2002"), "{c:?}");
}

#[test]
fn bool_arithmetic_e2003() {
    let c = codes("module M {\n    in  f : bool\n    in  g : bool\n    out y : bool\n\n    y = (f + g) == f\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

// ═══ Aritmetik: literal uyarlama ══════════════════════════════════

#[test]
fn literal_operand_adapts_to_concrete_side() {
    let result =
        check("module M {\n    in  a : u8\n    out y : u8\n\n    let _s = a + 1\n\n    y = a\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "_s"), "u9");
}

#[test]
fn literal_operand_overflow_e2010() {
    let c = codes(
        "module M {\n    in  a : u8\n    out y : u8\n\n    let _s = a + 300\n\n    y = a\n}\n",
    );
    assert!(c.contains(&"E2010"), "{c:?}");
}

#[test]
fn literal_plus_literal_stays_literal_then_w2012() {
    let result =
        check("module M {\n    in  a : u8\n    out y : u8\n\n    let _x = 1 + 2\n\n    y = a\n}\n");
    assert!(result.error_codes().contains(&"W2012"));
    assert_eq!(def_ty(&result, "_x"), "i32");
}

#[test]
fn literal_plus_bool_e2003() {
    let c = codes("module M {\n    in  f : bool\n    out y : bool\n\n    y = (1 + f) == 2\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

// ═══ Aritmetik: bits yasağı (E2004) ═══════════════════════════════

#[test]
fn bits_add_e2004() {
    let c = codes("module M {\n    in  a : bits<8>\n    in  b : bits<8>\n    out r : bits<8>\n\n    r = a + b\n}\n");
    assert!(c.contains(&"E2004"), "{c:?}");
}

#[test]
fn bits_with_numeric_arith_e2004() {
    let c = codes("module M {\n    in  a : bits<8>\n    in  b : u8\n    out r : u8\n\n    r = (a * b) as u8\n}\n");
    assert!(c.contains(&"E2004"), "{c:?}");
}

#[test]
fn bits_with_literal_arith_e2004() {
    let c = codes("module M {\n    in  a : bits<8>\n    out r : bits<8>\n\n    r = a + 1\n}\n");
    assert!(c.contains(&"E2004"), "{c:?}");
}

// ═══ Hata yayılımı ════════════════════════════════════════════════

#[test]
fn arith_with_unresolved_name_only_e1001() {
    let parsed = parse(
        FileId(0),
        "module M {\n    in  a : u8\n    out y : u9\n\n    y = a + bilinmeyen\n}\n",
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

#[test]
fn comparison_with_unresolved_name_only_e1001() {
    let parsed = parse(
        FileId(0),
        "module M {\n    in  a : u8\n    out y : bool\n\n    y = a == bilinmeyen\n}\n",
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

// ═══ Trit kuralları ═══════════════════════════════════════════════

#[test]
fn trit_mul_is_closed() {
    let c = codes(
        "module M {\n    in  t : Trit\n    in  w : Trit\n    out y : Trit\n\n    y = t * w\n}\n",
    );
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn trit_add_overflows_to_i3() {
    let c = codes(
        "module M {\n    in  t : Trit\n    in  w : Trit\n    out y : i3\n\n    y = t + w\n}\n",
    );
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn trit_sub_overflows_to_i3() {
    let c = codes(
        "module M {\n    in  t : Trit\n    in  w : Trit\n    out y : i3\n\n    y = t - w\n}\n",
    );
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn trit_div_e2003() {
    let c = codes(
        "module M {\n    in  t : Trit\n    in  w : Trit\n    out y : Trit\n\n    y = t / w\n}\n",
    );
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn trit_rem_e2003() {
    let c = codes(
        "module M {\n    in  t : Trit\n    in  w : Trit\n    out y : Trit\n\n    y = t % w\n}\n",
    );
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn ternary_mac_both_orders() {
    let c = codes("module M {\n    in  t : Trit\n    in  x : i8\n    out a : i8\n    out b : i8\n\n    a = t * x\n    b = x * t\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn trit_mul_unsigned_e2003() {
    let c =
        codes("module M {\n    in  t : Trit\n    in  x : u8\n    out y : u8\n\n    y = t * x\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn trit_add_signed_e2003() {
    let c =
        codes("module M {\n    in  t : Trit\n    in  x : i8\n    out y : i8\n\n    y = t + x\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn trit_mul_literal_adapts() {
    let c = codes("module M {\n    in  t : Trit\n    out y : Trit\n\n    y = t * 1\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn trit_arith_literal_out_of_set_e2011() {
    let c = codes("module M {\n    in  t : Trit\n    out y : Trit\n\n    y = t * 2\n}\n");
    assert!(c.contains(&"E2011"), "{c:?}");
}

// ═══ Bit düzeyi: genişlemez ═══════════════════════════════════════

#[test]
fn bitwise_ops_keep_width() {
    let c = codes("module M {\n    in  a : u8\n    in  b : u8\n    out x : u8\n    out y : u8\n    out z : u8\n\n    x = a & b\n    y = a | b\n    z = a ^ b\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn bitwise_bool_yields_bool() {
    let c = codes(
        "module M {\n    in  f : bool\n    in  g : bool\n    out y : bool\n\n    y = f & g\n}\n",
    );
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn bitwise_signed_keeps_width() {
    let result =
        check("module M {\n    in  a : i8\n    in  b : i8\n    out y : i8\n\n    y = a & b\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
}

#[test]
fn bitwise_bits_same_width_ok() {
    let c = codes("module M {\n    in  a : bits<8>\n    in  b : bits<8>\n    out y : bits<8>\n\n    y = a & b\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn bitwise_width_mismatch_e2001() {
    let c =
        codes("module M {\n    in  a : u8\n    in  b : u16\n    out y : u16\n\n    y = a & b\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn bitwise_bits_width_mismatch_e2001() {
    let c = codes("module M {\n    in  a : bits<8>\n    in  b : bits<4>\n    out y : bits<8>\n\n    y = a & b\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn bitwise_sign_mismatch_e2002() {
    let c =
        codes("module M {\n    in  a : u8\n    in  b : i8\n    out y : u8\n\n    y = a & b\n}\n");
    assert!(c.contains(&"E2002"), "{c:?}");
}

#[test]
fn bitwise_does_not_widen_e2001() {
    // u8 & u8 → u8; u9 hedefe örtük genişlemez.
    let c =
        codes("module M {\n    in  a : u8\n    in  b : u8\n    out y : u9\n\n    y = a & b\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn bitwise_literal_mask_adapts() {
    let c = codes("module M {\n    in  a : u8\n    out y : bool\n\n    y = a & 0x0F == 0\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn bitwise_on_flex_arith_result_wraps() {
    // (a + b) esnek [8,9]; u8 maske ile kesişim u8 — sonuç u8.
    let c = codes("module M {\n    in  a : u8\n    in  b : u8\n    in  m : u8\n    out y : u8\n\n    y = (a + b) & m\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn bitwise_bool_with_uint_e2003() {
    let c =
        codes("module M {\n    in  f : bool\n    in  a : u8\n    out y : u8\n\n    y = f & a\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn bitwise_trit_e2003() {
    let c = codes(
        "module M {\n    in  t : Trit\n    in  w : Trit\n    out y : Trit\n\n    y = t & w\n}\n",
    );
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn bitwise_bits_with_literal_e2003() {
    // bits sayısal değildir; maske için açık dönüşüm gerekir.
    let c = codes("module M {\n    in  a : bits<8>\n    out y : bits<8>\n\n    y = a & 1\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

// ═══ Kaydırma ═════════════════════════════════════════════════════

#[test]
fn shift_keeps_left_operand_type() {
    let c = codes("module M {\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    y = (a << 2) | (b >> 1)\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn shift_signed_keeps_type() {
    let c = codes("module M {\n    in  a : i8\n    out y : i8\n\n    y = a >> 2\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn shift_bits_keeps_type() {
    let c = codes("module M {\n    in  a : bits<8>\n    out y : bits<8>\n\n    y = a << 3\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn shift_by_runtime_amount_ok() {
    let c =
        codes("module M {\n    in  a : u8\n    in  n : u3\n    out y : u8\n\n    y = a << n\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn shift_amount_not_numeric_e2003() {
    let c = codes(
        "module M {\n    in  a : u8\n    in  f : bool\n    out y : u8\n\n    y = a << f\n}\n",
    );
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn shift_amount_bits_e2003() {
    let c = codes(
        "module M {\n    in  a : u8\n    in  n : bits<3>\n    out y : u8\n\n    y = a << n\n}\n",
    );
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn shift_of_bool_e2003() {
    let c = codes("module M {\n    in  f : bool\n    out y : bool\n\n    y = (f << 1) == f\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn shift_amount_at_width_w2013() {
    let result = check("module M {\n    in  a : u8\n    out y : u8\n\n    y = a << 8\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert!(result.error_codes().contains(&"W2013"));
}

#[test]
fn shift_amount_below_width_no_w2013() {
    let result = check("module M {\n    in  a : u8\n    out y : u8\n\n    y = a << 7\n}\n");
    assert!(!result.error_codes().contains(&"W2013"));
}

#[test]
fn shift_amount_from_const_w2013() {
    let result = check(
        "const N : u32 = 9;\n\nmodule M {\n    in  a : u8\n    out y : u8\n\n    y = a >> N\n}\n",
    );
    assert!(result.error_codes().contains(&"W2013"));
}

// ═══ Karşılaştırma ════════════════════════════════════════════════

#[test]
fn all_comparison_ops_yield_bool() {
    let c = codes("module M {\n    in  a : u8\n    in  b : u8\n    out o1 : bool\n    out o2 : bool\n    out o3 : bool\n    out o4 : bool\n    out o5 : bool\n    out o6 : bool\n\n    o1 = a == b\n    o2 = a != b\n    o3 = a < b\n    o4 = a > b\n    o5 = a <= b\n    o6 = a >= b\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn comparison_width_mismatch_e2003() {
    let c = codes(
        "module M {\n    in  a : u8\n    in  b : u16\n    out y : bool\n\n    y = a < b\n}\n",
    );
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn comparison_sign_mismatch_e2003() {
    let c = codes(
        "module M {\n    in  a : u8\n    in  b : i8\n    out y : bool\n\n    y = a == b\n}\n",
    );
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn comparison_diag_shows_both_types() {
    let result = check(
        "module M {\n    in  a : u8\n    in  b : u16\n    out y : bool\n\n    y = a == b\n}\n",
    );
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E2003")
        .expect("E2003 bekleniyor");
    assert!(diag.message.contains("u8") && diag.message.contains("u16"));
}

#[test]
fn comparison_literal_adapts() {
    let c = codes("module M {\n    in  a : u8\n    out y : bool\n\n    y = a == 5\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn comparison_literal_overflow_e2010() {
    let c = codes("module M {\n    in  a : u8\n    out y : bool\n\n    y = a == 300\n}\n");
    assert!(c.contains(&"E2010"), "{c:?}");
}

#[test]
fn comparison_bool_operands_ok() {
    let c = codes(
        "module M {\n    in  f : bool\n    in  g : bool\n    out y : bool\n\n    y = f == g\n}\n",
    );
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn comparison_trit_operands_ok() {
    let c = codes(
        "module M {\n    in  t : Trit\n    in  w : Trit\n    out y : bool\n\n    y = t != w\n}\n",
    );
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn comparison_bool_with_uint_e2003() {
    let c = codes(
        "module M {\n    in  f : bool\n    in  a : u8\n    out y : bool\n\n    y = f == a\n}\n",
    );
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn comparison_of_flex_result_with_same_width_ok() {
    // (a + b) esnek [8,9] ile u8 kesişir — karşılaştırma geçerli.
    let c = codes("module M {\n    in  a : u8\n    in  b : u8\n    in  m : u8\n    out y : bool\n\n    y = (a + b) == m\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

// ═══ Mantıksal ════════════════════════════════════════════════════

#[test]
fn logical_and_or_on_bool_ok() {
    let c = codes("module M {\n    in  f : bool\n    in  g : bool\n    out y : bool\n    out z : bool\n\n    y = f && g\n    z = f || g\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn logical_on_uint_e2003() {
    let c = codes(
        "module M {\n    in  a : u8\n    in  f : bool\n    out y : bool\n\n    y = a && f\n}\n",
    );
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn logical_with_literal_e2003() {
    let c = codes("module M {\n    in  f : bool\n    out y : bool\n\n    y = f || 1\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn logical_of_comparisons_ok() {
    let c = codes("module M {\n    in  a : u8\n    in  b : u8\n    out y : bool\n\n    y = (a < b) && (a != 0)\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

// ═══ Koşullu ifade (§3.7) ═════════════════════════════════════════

#[test]
fn if_expr_branch_type_mismatch_e2003_shows_both() {
    // Sentez konumunda (let) dallar birleştirilir; uyumsuzluk E2003.
    let result = check("module M {\n    in  s : bool\n    in  a : u8\n    in  b : u16\n    out y : u8\n\n    let _v = if s { a } else { b }\n\n    y = a\n}\n");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E2003")
        .expect("E2003 bekleniyor");
    assert!(diag.message.contains("u8") && diag.message.contains("u16"));
}

#[test]
fn if_expr_bool_vs_uint_e2003() {
    let c = codes("module M {\n    in  s : bool\n    in  a : u8\n    out y : u8\n\n    y = if s { a } else { s }\n}\n");
    assert!(c.contains(&"E2003"), "{c:?}");
}

#[test]
fn if_expr_flex_branch_unifies_with_concrete() {
    // then: esnek u9, else: u8 — kesişim u8; hedef u8 temiz.
    let c = codes("module M {\n    in  s : bool\n    in  a : u8\n    in  b : u8\n    out y : u8\n\n    y = if s { a + b } else { a }\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn if_expr_literal_else_adapts_to_then() {
    let c = codes("module M {\n    in  s : bool\n    in  a : u8\n    out y : u8\n\n    y = if s { a } else { 0 }\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

// ═══ Bileşimler ═══════════════════════════════════════════════════

#[test]
fn arith_result_can_be_indexed() {
    // (a + b)[3] — esnek sonuç bit seçimine girer, bool döner.
    let c = codes(
        "module M {\n    in  a : u8\n    in  b : u8\n    out y : bool\n\n    y = (a + b)[3]\n}\n",
    );
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn negation_of_flex_result_widens_from_natural() {
    // -(a + b): esnek i9 doğal genişliğinden i10'a genişler.
    let result = check(
        "module M {\n    in  a : i8\n    in  b : i8\n    out y : i8\n\n    let _n = -(a + b)\n\n    y = a\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "_n"), "i10");
}

#[test]
fn mul_result_wraps_to_operand_width() {
    // ADR-0025: çarpım da operand genişliğine kırpılabilir (alt yarı).
    let c =
        codes("module M {\n    in  a : u8\n    in  b : u8\n    out p : u8\n\n    p = a * b\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

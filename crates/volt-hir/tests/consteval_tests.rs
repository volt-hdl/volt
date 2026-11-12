//! Const eval testleri (const-eval.md §12 test vektörleri).

use volt_hir::{analyze, ConstEvaluator, ConstValue, MAX_WIDTH};
use volt_syntax::{parse, FileId};

/// `const SONUC : u32 = <expr>;` biçimindeki kaynakta SONUC'u değerlendirir.
fn eval_named(src: &str, name: &str) -> ConstValue {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "test kaynağı ayrışmalı: {:?}",
        parsed.error_codes()
    );
    let res = volt_hir::resolve_file(&parsed.ast);
    assert!(
        res.diagnostics.iter().all(|d| d.code.is_warning()),
        "çözümleme hatasız olmalı: {:?}",
        res.error_codes()
    );
    let mut ev = ConstEvaluator::new(&parsed.ast, &res);
    let (def, _) = res.def_by_name(name).expect("sabit tanımlı olmalı");
    let value = ev.eval_const_def(def);
    assert!(
        ev.diagnostics.is_empty(),
        "değerlendirme hatasız olmalı: {:?}",
        ev.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    value
}

fn eval_expr(expr: &str) -> ConstValue {
    eval_named(&format!("const SONUC : u32 = {expr};"), "SONUC")
}

/// Analiz koşturup tanı kodlarını döndürür (hata senaryoları).
fn codes(src: &str) -> Vec<&'static str> {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "test kaynağı ayrışmalı: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast).error_codes()
}

// ═══ Değerlendirilebilir ifadeler (§12: sonuç) ════════════════════

#[test]
fn int_literal() {
    assert_eq!(eval_expr("42"), ConstValue::Int(42));
}

#[test]
fn precedence_mul_before_add() {
    assert_eq!(eval_expr("2 + 3 * 4"), ConstValue::Int(14));
}

#[test]
fn shift_left() {
    assert_eq!(eval_expr("1 << 8"), ConstValue::Int(256));
}

#[test]
fn subtraction_and_division() {
    assert_eq!(eval_expr("(10 - 4) / 2"), ConstValue::Int(3));
}

#[test]
fn remainder() {
    assert_eq!(eval_expr("7 % 3"), ConstValue::Int(1));
}

#[test]
fn bitwise_ops() {
    assert_eq!(eval_expr("12 & 10"), ConstValue::Int(8));
    assert_eq!(eval_expr("12 | 10"), ConstValue::Int(14));
    assert_eq!(eval_expr("12 ^ 10"), ConstValue::Int(6));
}

#[test]
fn unary_neg_and_bitnot() {
    assert_eq!(eval_expr("0 - 5"), ConstValue::Int(-5));
    assert_eq!(eval_expr("~0"), ConstValue::Int(-1));
}

#[test]
fn comparison_yields_bool() {
    assert_eq!(
        eval_named("const SONUC : bool = 3 < 5;", "SONUC"),
        ConstValue::Bool(true)
    );
}

#[test]
fn logical_ops_on_bool() {
    assert_eq!(
        eval_named("const SONUC : bool = true && false;", "SONUC"),
        ConstValue::Bool(false)
    );
    assert_eq!(
        eval_named("const SONUC : bool = true || false;", "SONUC"),
        ConstValue::Bool(true)
    );
    assert_eq!(
        eval_named("const SONUC : bool = !true;", "SONUC"),
        ConstValue::Bool(false)
    );
}

#[test]
fn implication_on_bool() {
    // a -> b ≡ !a || b (ADR-0034): yalnız true -> false sonucu false.
    assert_eq!(
        eval_named("const SONUC : bool = true -> false;", "SONUC"),
        ConstValue::Bool(false)
    );
    assert_eq!(
        eval_named("const SONUC : bool = false -> false;", "SONUC"),
        ConstValue::Bool(true)
    );
    assert_eq!(
        eval_named("const SONUC : bool = true -> true;", "SONUC"),
        ConstValue::Bool(true)
    );
}

#[test]
fn if_with_constant_condition() {
    assert_eq!(eval_expr("if true { 1 } else { 2 }"), ConstValue::Int(1));
    assert_eq!(eval_expr("if 3 > 5 { 1 } else { 2 }"), ConstValue::Int(2));
}

#[test]
fn array_literal_index() {
    assert_eq!(eval_expr("[1, 2, 3][1]"), ConstValue::Int(2));
}

#[test]
fn array_repeat() {
    assert_eq!(
        eval_named("const SONUC : [u32; 4] = [0; 4];", "SONUC"),
        ConstValue::Array(vec![ConstValue::Int(0); 4])
    );
}

#[test]
fn cast_preserves_value() {
    assert_eq!(eval_expr("(8 as u16)"), ConstValue::Int(8));
}

#[test]
fn const_reference() {
    assert_eq!(
        eval_named(
            "const WIDTH : u32 = 8;\nconst SONUC : u32 = WIDTH * 2;",
            "SONUC"
        ),
        ConstValue::Int(16)
    );
}

#[test]
fn forward_const_reference() {
    // İleri referans döngü değildir.
    assert_eq!(
        eval_named("const SONUC : u32 = B + 1;\nconst B : u32 = 2;", "SONUC"),
        ConstValue::Int(3)
    );
}

#[test]
fn enum_variant_explicit_discriminant() {
    let v = eval_named(
        "enum Durum : bits<2> { Bekle = 0, Calis = 1 }\nconst SONUC : Durum = Durum::Calis;",
        "SONUC",
    );
    let ConstValue::EnumVariant { discriminant, .. } = v else {
        panic!("EnumVariant bekleniyor: {v:?}");
    };
    assert_eq!(discriminant, 1);
}

#[test]
fn enum_variant_positional_discriminant() {
    let v = eval_named(
        "enum Durum : bits<2> { Bekle = 0, Calis = 1, Bitti }\nconst SONUC : Durum = Durum::Bitti;",
        "SONUC",
    );
    let ConstValue::EnumVariant { discriminant, .. } = v else {
        panic!("EnumVariant bekleniyor: {v:?}");
    };
    assert_eq!(discriminant, 2);
}

// ═══ clog2 (§6) ═══════════════════════════════════════════════════

#[test]
fn clog2_of_one_is_zero() {
    assert_eq!(eval_expr("clog2(1)"), ConstValue::Int(0));
}

#[test]
fn clog2_rounds_up() {
    assert_eq!(eval_expr("clog2(3)"), ConstValue::Int(2));
}

#[test]
fn clog2_of_64_is_6() {
    assert_eq!(eval_expr("clog2(64)"), ConstValue::Int(6));
}

#[test]
fn clog2_powers_of_two() {
    assert_eq!(eval_expr("clog2(2)"), ConstValue::Int(1));
    assert_eq!(eval_expr("clog2(8)"), ConstValue::Int(3));
    assert_eq!(eval_expr("clog2(256)"), ConstValue::Int(8));
}

#[test]
fn zext_passes_value_through() {
    assert_eq!(eval_expr("zext(4)"), ConstValue::Int(4));
    assert_eq!(eval_expr("sext(4)"), ConstValue::Int(4));
    assert_eq!(eval_expr("trunc(4)"), ConstValue::Int(4));
}

// ═══ Hata durumları (§12) ═════════════════════════════════════════

#[test]
fn division_by_zero_e2023() {
    let codes = codes("const X : u32 = 1 / 0;");
    assert!(codes.contains(&"E2023"), "{codes:?}");
}

#[test]
fn remainder_by_zero_e2023() {
    let codes = codes("const X : u32 = 1 % 0;");
    assert!(codes.contains(&"E2023"), "{codes:?}");
}

#[test]
fn shift_amount_too_large_e2024() {
    let codes = codes("const HUGE : u32 = 1 << 200;");
    assert!(codes.contains(&"E2024"), "{codes:?}");
}

#[test]
fn negative_shift_e2024() {
    let codes = codes("const X : u32 = 1 << (0 - 1);");
    assert!(codes.contains(&"E2024"), "{codes:?}");
}

#[test]
fn multiplication_overflow_e2022() {
    let codes = codes("const X : u32 = (1 << 126) * 4;");
    assert!(codes.contains(&"E2022"), "{codes:?}");
}

#[test]
fn cyclic_dependency_e2020() {
    let codes = codes("const A : u32 = B + 1;\nconst B : u32 = A + 1;");
    assert_eq!(
        codes.iter().filter(|c| **c == "E2020").count(),
        1,
        "tam bir E2020 bekleniyor: {codes:?}"
    );
}

#[test]
fn self_cycle_e2020() {
    let codes = codes("const A : u32 = A + 1;");
    assert!(codes.contains(&"E2020"), "{codes:?}");
}

#[test]
fn runtime_value_in_type_e2021() {
    let codes = codes("module M { in genislik : u8 in data : bits<genislik> out y : u8 y = 0 }");
    assert!(codes.contains(&"E2021"), "{codes:?}");
}

#[test]
fn width_zero_e2025() {
    let codes = codes("module M { in data : bits<0> out y : u8 y = 0 }");
    assert!(codes.contains(&"E2025"), "{codes:?}");
}

#[test]
fn width_negative_e2025() {
    let codes = codes("module M { in data : bits<0 - 1> out y : u8 y = 0 }");
    assert!(codes.contains(&"E2025"), "{codes:?}");
}

#[test]
fn width_too_large_e2025() {
    let codes = codes(&format!(
        "module M {{ in data : bits<{}> out y : u8 y = 0 }}",
        MAX_WIDTH + 1
    ));
    assert!(codes.contains(&"E2025"), "{codes:?}");
}

#[test]
fn max_width_is_valid() {
    let codes = codes(&format!(
        "module M {{ in data : bits<{MAX_WIDTH}> out y : u8 y = 0 }}"
    ));
    assert!(!codes.contains(&"E2025"), "{codes:?}");
}

#[test]
fn array_len_over_limit_e2026() {
    let codes = codes("module M { in data : [u8; 2000000] out y : u8 y = 0 }");
    assert!(codes.contains(&"E2026"), "{codes:?}");
}

#[test]
fn repeat_count_over_limit_e2026() {
    let codes = codes("const X : [u32; 2000000] = [0; 2000000];");
    assert!(codes.contains(&"E2026"), "{codes:?}");
}

#[test]
fn const_array_index_out_of_bounds_e2029() {
    let codes = codes("const X : u32 = [1, 2, 3][5];");
    assert!(codes.contains(&"E2029"), "{codes:?}");
}

#[test]
fn caching_reports_error_once() {
    // A'nın hatası önbelleklenir; B'nin iki başvurusu yeni tanı üretmez.
    let codes = codes("const A : u32 = 1 / 0;\nconst B : u32 = A + A;");
    assert_eq!(
        codes.iter().filter(|c| **c == "E2023").count(),
        1,
        "önbellek yinelenen tanıyı engellemeli: {codes:?}"
    );
}

#[test]
fn bool_arithmetic_type_mismatch() {
    let codes = codes("const X : u32 = true + false;");
    assert!(codes.contains(&"E2003"), "{codes:?}");
}

// ═══ i128 aralığının sınırları (#207) ═════════════════════════════
//
// const-eval.md §1: değerlendiricinin tam sayısı `i128`'dir, §4: aralık
// dışı değer taşmadır (E2022), sarma değil. Literal ise 128 bitlik
// işaretsiz sayıdır (`IntLit { value: u128 }`, ast-nodes.md): 2^127 ve
// üstü `i128`'e sığmaz. Düzeltmeden önce değerlendirici literali
// `value as i128` ile sarıyordu (2^127 → i128::MIN, 2^128 - 1 → -1) ve
// -2^127'yi (i128::MIN, aralıkta) E2022 sayıyordu.

/// Değerlendiricinin değeri ve kendi tanı kodları (tip denetimi koşmaz).
fn eval_with_codes(expr: &str) -> (ConstValue, Vec<String>) {
    let src = format!("const SONUC : u32 = {expr};");
    let parsed = parse(FileId(0), &src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    let res = volt_hir::resolve_file(&parsed.ast);
    let mut ev = ConstEvaluator::new(&parsed.ast, &res);
    let (def, _) = res.def_by_name("SONUC").expect("sabit tanımlı olmalı");
    let value = ev.eval_const_def(def);
    let codes = ev
        .diagnostics
        .iter()
        .map(|d| d.code.as_str().to_string())
        .collect();
    (value, codes)
}

#[test]
fn issue_207_i128_max_literal_keeps_its_value() {
    assert_eq!(
        eval_expr("170141183460469231731687303715884105727"),
        ConstValue::Int(i128::MAX)
    );
}

#[test]
fn issue_207_minus_two_pow_127_is_i128_min() {
    assert_eq!(
        eval_expr("-170141183460469231731687303715884105728"),
        ConstValue::Int(i128::MIN)
    );
    assert_eq!(
        eval_expr("-0x8000_0000_0000_0000_0000_0000_0000_0000 + 1"),
        ConstValue::Int(i128::MIN + 1)
    );
}

#[test]
fn issue_207_two_pow_127_literal_is_an_overflow_not_i128_min() {
    let (value, codes) = eval_with_codes("170141183460469231731687303715884105728");
    assert_eq!(value, ConstValue::Error);
    assert_eq!(codes, ["E2022"]);
}

#[test]
fn issue_207_two_pow_128_minus_one_is_an_overflow_not_minus_one() {
    // Sarılmış okuma: -1 / (2^64 + 1) = 0. Matematikte 2^64 - 1.
    let (value, codes) =
        eval_with_codes("0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF / 0x1_0000_0000_0000_0001");
    assert_eq!(value, ConstValue::Error);
    assert_eq!(codes, ["E2022"]);
}

#[test]
fn issue_207_negated_literal_beyond_two_pow_127_is_an_overflow() {
    // -(2^127 + 1) i128'in altında; sarılmış okuma -(i128::MIN + 1).
    let (value, codes) = eval_with_codes("-0x8000_0000_0000_0000_0000_0000_0000_0001");
    assert_eq!(value, ConstValue::Error);
    assert_eq!(codes, ["E2022"]);
}

#[test]
fn issue_207_one_below_i128_min_is_an_overflow() {
    // Denetim: i128::MIN - 1 çıkarmanın taşmasıdır (düzeltmeden önce de
    // E2022, ama -2^127'nin kendisi yüzünden).
    let (value, codes) = eval_with_codes("-170141183460469231731687303715884105728 - 1");
    assert_eq!(value, ConstValue::Error);
    assert_eq!(codes, ["E2022"]);
}

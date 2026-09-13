//! ADR-0041 — açık hedef tipli aynı-işaret örtük genişleme.
//!
//! İlke: hedef tip AÇIKÇA yazılmışsa (let/reg/port tipi, atama hedefi,
//! port bağlama) ve işaret aynıysa, daha dar bir değer hedefe örtük
//! genişler; aritmetikte beklenen tip operandlara itilir ("önce genişlet,
//! sonra işle"). Daraltma HÂLÂ E2001, işaret farkı HÂLÂ E2002, hedefsiz
//! (`let c = a + b`) operand genişlik farkı HÂLÂ E2001.

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

// ═══ Genişleme serbest (hedef açık, işaret aynı) ══════════════════

#[test]
fn let_with_explicit_wider_type_widens_add_operands() {
    // Arrange: a, b i18; hedef i19 (taşıma biti korunur)
    let src = "module M {\n    in  a : i18\n    in  b : i18\n    out y : i19\n\n    let acc : i19 = a + b\n\n    y = acc\n}\n";
    // Act
    let result = check(src);
    // Assert
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "acc"), "i19");
}

#[test]
fn mixed_operand_widths_widen_to_explicit_target() {
    // a18 + b19 → i20 hedefte iki operand da i20'ye genişler.
    let c = codes("module M {\n    in  a : i18\n    in  b : i19\n    out y : i20\n\n    let c : i20 = a + b\n\n    y = c\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn mixed_operand_widths_widen_to_wider_operand_target() {
    // a18 + b19 → i19 hedef: a genişler, b eşit; sonuç esnek [19,20] sığar.
    let c = codes("module M {\n    in  a : i18\n    in  b : i19\n    out y : i19\n\n    let c : i19 = a + b\n\n    y = c\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn mixed_operand_widths_without_target_still_e2001() {
    // Hedef yazılmamış: operand genişlikleri örtük birleştirilemez.
    let c = codes("module M {\n    in  a : i18\n    in  b : i19\n    out y : i19\n\n    let c = a + b\n\n    y = a\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn multiply_widens_operands_to_i32_target() {
    // FIR deseni: i16 tap × i16 katsayı sabiti, i32 hedef — cast'siz.
    let c = codes("const C : i16 = 3\n\nmodule M {\n    in  t0 : i16\n    out y : i32\n\n    let p : i32 = t0 * C\n\n    y = p\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn nested_products_sum_widens_to_i32_target() {
    // (a*b) + (c*d) — çarpımların doğal genişliği 32, hedefe sığar.
    let c = codes("module M {\n    in  a : i16\n    in  b : i16\n    in  c : i16\n    in  d : i16\n    out y : i32\n\n    let s : i32 = (a * b) + (c * d)\n\n    y = s\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn plain_signal_widens_to_explicit_let_type() {
    let result = check(
        "module M {\n    in  y : i16\n    out o : i32\n\n    let x : i32 = y\n\n    o = x\n}\n",
    );
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    assert_eq!(def_ty(&result, "x"), "i32");
}

#[test]
fn output_port_assignment_widens_narrower_source() {
    // result = sum — port i32, kaynak i16.
    let c = codes("module M {\n    in  sum : i16\n    out result : i32\n\n    result = sum\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn register_nonblocking_assign_widens_source() {
    let c = codes("module M {\n    in  clk : clock\n    in  s : u8\n    out y : u16\n\n    reg r : u16 = 0\n\n    on clk { r <= s }\n\n    y = r\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn counter_pattern_still_adapts_to_operand_width() {
    // count <= count + 1 — hedef u8, operandlar u8: taşma biti atılır.
    let c = codes("module M {\n    in  clk : clock\n    out y : u8\n\n    reg count : u8 = 0\n\n    on clk { count <= count + 1 }\n\n    y = count\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn literal_operand_adapts_to_widened_target() {
    // a u8; hedef u16: a genişler, 300 literali u16'ya sığar.
    let c = codes("module M {\n    in  a : u8\n    out y : u16\n\n    let s : u16 = a + 300\n\n    y = s\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn literal_operand_overflow_against_target_e2010() {
    // Genişletilmiş hedef u9 bile 1000 literalini almaz.
    let c = codes(
        "module M {\n    in  a : u8\n    out y : u9\n\n    let s : u9 = a + 1000\n\n    y = s\n}\n",
    );
    assert!(c.contains(&"E2010"), "{c:?}");
}

#[test]
fn negation_result_widens_to_explicit_target() {
    // -x (i16) → i17 doğal; i32 hedefe sığar.
    let c = codes(
        "module M {\n    in  x : i16\n    out o : i32\n\n    let y : i32 = -x\n\n    o = y\n}\n",
    );
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn instance_input_binding_widens_narrower_source() {
    let c = codes("module Adder {\n    in  a : u16\n    out s : u16\n\n    s = a\n}\n\nmodule Top {\n    in  x : u8\n    out y : u16\n\n    let add = Adder { a: x }\n\n    y = add.s\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn widened_operands_are_recorded_with_target_type() {
    // Operand ifadelerinin kayıtlı tipi genişletilmiş hedef olmalı
    // (araçlar/SV üretimi "önce genişlet" semantiğini görebilsin).
    let result = check("module M {\n    in  a : i18\n    in  b : i18\n    out y : i19\n\n    let acc : i19 = a + b\n\n    y = acc\n}\n");
    assert!(!result.has_errors(), "{:?}", result.error_codes());
    let widened = result
        .typeck
        .expr_types
        .values()
        .filter(|&&t| result.typeck.types.display(t) == "i19")
        .count();
    // a, b ve a+b: en az üç i19 kaydı.
    assert!(widened >= 3, "i19 kayıt sayısı {widened}");
}

// ═══ İlke korunuyor: daraltma E2001, işaret farkı E2002 ══════════

#[test]
fn narrowing_let_still_e2001() {
    let c = codes(
        "module M {\n    in  u : i16\n    out y : i16\n\n    let v : i8 = u\n\n    y = u\n}\n",
    );
    assert!(c.contains(&"E2001"), "{c:?}");
}

#[test]
fn narrowing_arith_result_still_e2001() {
    // a16 + b16 → i8 hedef: operandlar hedefe sığmaz, sonuç daraltılamaz.
    let c = codes("module M {\n    in  a : i16\n    in  b : i16\n    out y : i16\n\n    let v : i8 = a + b\n\n    y = a\n}\n");
    assert!(c.contains(&"E2001"), "{c:?}");
    assert!(!c.contains(&"E2002"), "{c:?}");
}

#[test]
fn narrowing_diag_says_does_not_fit_with_reason_note() {
    let result = check("module M {\n    in  u : i16\n    out y : i8\n\n    y = u\n}\n");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "E2001")
        .expect("E2001 bekleniyor");
    assert!(
        diag.message.contains("16") && diag.message.contains("8"),
        "{}",
        diag.message
    );
    assert!(!diag.help.as_deref().unwrap_or("").is_empty());
    assert!(!diag.notes.is_empty(), "neden notu taşımalı");
}

#[test]
fn sign_mismatch_widening_still_e2002() {
    // i16 → u16: genişlik eşit, işaret farklı.
    let c = codes(
        "module M {\n    in  w : i16\n    out y : i16\n\n    let z : u16 = w\n\n    y = w\n}\n",
    );
    assert!(c.contains(&"E2002"), "{c:?}");
}

#[test]
fn sign_mismatch_wider_target_still_e2002() {
    // i8 → u16: daha geniş hedef bile işaret farkını affetmez.
    let c = codes("module M {\n    in  w : i8\n    out y : u16\n\n    y = w\n}\n");
    assert!(c.contains(&"E2002"), "{c:?}");
    assert!(!c.contains(&"E2001"), "{c:?}");
}

#[test]
fn sign_mismatch_arith_operand_against_target_e2002() {
    // u8 + u8 sonucu i16 hedefe: genişleme yolu işaret farkında kapalı.
    let c =
        codes("module M {\n    in  a : u8\n    in  b : u8\n    out y : i16\n\n    y = a + b\n}\n");
    assert!(c.contains(&"E2002"), "{c:?}");
}

// ═══ Genişleme yolunun kapsamı dışı: mevcut davranış korunur ═════

#[test]
fn trit_addition_to_i3_target_keeps_trit_rule() {
    let c = codes("module M {\n    in  t1 : Trit\n    in  t2 : Trit\n    out y : i3\n\n    let z : i3 = t1 + t2\n\n    y = z\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn bits_arithmetic_with_target_still_e2004() {
    let c = codes("module M {\n    in  a : bits<8>\n    in  b : bits<8>\n    out y : u8\n\n    let s : u16 = a + b\n\n    y = a as u8\n}\n");
    assert!(c.contains(&"E2004"), "{c:?}");
}

#[test]
fn literal_only_arithmetic_with_target_ok() {
    let c = codes("module M {\n    out y : u8\n\n    let s : u8 = 1 + 2\n\n    y = s\n}\n");
    assert!(c.is_empty(), "{c:?}");
}

#[test]
fn widening_does_not_duplicate_operand_diagnostics() {
    // Uygun olmayan yol (b hedeften geniş) operandları bir kez sentezler:
    // tanımsız isim E1001 tek sefer raporlanır.
    let result = check(
        "module M {\n    in  a : u8\n    out y : u8\n\n    let s : u8 = a + nope\n\n    y = a\n}\n",
    );
    let e1001 = result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E1001")
        .count();
    assert_eq!(e1001, 1, "{:?}", result.error_codes());
}

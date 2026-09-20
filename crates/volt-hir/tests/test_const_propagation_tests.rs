//! Test bloğunda sabit yayılımı (ADR-0060): `let` ile bağlanan sabitler
//! port genişliği denetimine (E8512) derleme zamanında ulaşır; çalışma
//! zamanı değerleri tanı üretmez (testbench koşuda denetler).

use volt_ast::SourceFile;
use volt_diagnostics::Diagnostic;
use volt_hir::check_tests;

fn parse(src: &str) -> SourceFile {
    let parsed = volt_syntax::parser::parse(volt_span::FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "ayrışmalı: {:?}",
        parsed.error_codes()
    );
    parsed.ast
}

const DUT: &str = "\
const LIMIT : u8 = 8
const SMALL : u8 = 7
const DERIVED : u8 = SMALL + 1

module Dut {
    in  clk  : clock
    in  addr : u3
    in  sv   : i8
    out data : u8
    reg r : u8 = 0
    on clk { r <= r + 1 }
    data = r
}
";

fn diags(body: &str) -> Vec<Diagnostic> {
    let src = format!("{DUT}\ntest \"t\" {{\n    let dut = Dut {{ }};\n{body}\n}}\n");
    let ast = parse(&src);
    check_tests(&[&ast], &ast, false)
}

fn codes(body: &str) -> Vec<String> {
    diags(body)
        .iter()
        .map(|d| d.code.as_str().to_string())
        .collect()
}

// ═══ Kapsananlar ══════════════════════════════════════════════════

#[test]
fn literal_let_reaches_the_port_check() {
    assert_eq!(codes("let n = 8;\ndut.addr = n;"), vec!["E8512"]);
    assert!(codes("let n = 7;\ndut.addr = n;").is_empty());
}

#[test]
fn let_chain_is_folded() {
    assert_eq!(
        codes("let n = 7;\nlet m = n + 1;\ndut.addr = m;"),
        vec!["E8512"]
    );
    assert!(codes("let n = 8;\nlet m = n - 1;\ndut.addr = m;").is_empty());
}

#[test]
fn expression_over_constants_at_the_use_site_is_folded() {
    assert_eq!(codes("let n = 4;\ndut.addr = n * 2;"), vec!["E8512"]);
    assert!(codes("let n = 4;\ndut.addr = n * 2 - 1;").is_empty());
}

#[test]
fn top_level_literal_const_is_usable_and_checked() {
    assert_eq!(codes("let k = LIMIT;\ndut.addr = k;"), vec!["E8512"]);
    assert_eq!(codes("dut.addr = LIMIT;"), vec!["E8512"]);
    assert!(codes("let k = SMALL;\ndut.addr = k;").is_empty());
}

#[test]
fn signed_port_accepts_a_propagated_negative() {
    assert!(codes("let neg = 0 - 128;\ndut.sv = neg;").is_empty());
    assert_eq!(codes("let neg = 0 - 129;\ndut.sv = neg;"), vec!["E8512"]);
}

#[test]
fn assert_against_an_unreachable_propagated_constant_is_e8512() {
    assert_eq!(
        codes("let big = 300;\nassert_eq(dut.data, big);"),
        vec!["E8512"]
    );
    assert!(codes("let top = 255;\nassert_eq(dut.data, top);").is_empty());
}

#[test]
fn constant_let_inside_a_loop_is_still_constant() {
    // Her yinelemede yeniden bağlanır ama değeri sayaca bağlı değildir.
    assert_eq!(
        codes("for i in 0..2 {\n    let n = 8;\n    dut.addr = n;\n}"),
        vec!["E8512"]
    );
}

#[test]
fn constant_index_is_bounds_checked() {
    assert_eq!(
        codes("let t = [1, 2];\nlet i = 2;\ndut.addr = t[i];"),
        vec!["E8511"]
    );
    assert!(codes("let t = [1, 2];\nlet i = 1;\ndut.addr = t[i];").is_empty());
}

// ═══ Kapsanmayanlar: tanı yok, koşuda denetlenir ══════════════════

#[test]
fn port_read_is_not_constant() {
    assert!(codes("let x = dut.data;\ndut.addr = x;").is_empty());
    assert!(codes("let x = dut.data;\nlet y = x + 100;\ndut.addr = y;").is_empty());
}

#[test]
fn loop_counter_is_not_constant() {
    assert!(codes("for i in 0..16 {\n    dut.addr = i;\n}").is_empty());
    assert!(codes("for i in 0..16 {\n    let n = i + 8;\n    dut.addr = n;\n}").is_empty());
}

#[test]
fn loop_counter_shadows_a_top_level_const() {
    assert!(codes("for LIMIT in 0..4 {\n    dut.addr = LIMIT;\n}").is_empty());
}

#[test]
fn array_element_and_len_are_not_constant() {
    assert!(codes("let t = [9, 9];\ndut.addr = t[0];").is_empty());
    assert!(codes("let t = [1, 2, 3, 4, 5, 6, 7, 8, 9];\ndut.addr = len(t);").is_empty());
}

#[test]
fn division_by_a_constant_zero_is_not_constant() {
    assert!(codes("let z = 0;\nlet bad = 9 / z;\ndut.addr = bad;").is_empty());
}

// ═══ Sessiz kalmayan sınırlar ═════════════════════════════════════

#[test]
fn computed_top_level_const_is_rejected_not_ignored() {
    let found = diags("dut.addr = DERIVED;");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].code.as_str(), "E8506");
    assert!(found[0].message.contains("DERIVED"), "{found:?}");
}

#[test]
fn undefined_name_is_still_e8506() {
    assert_eq!(codes("dut.addr = nowhere;"), vec!["E8506"]);
}

#[test]
fn rebinding_is_rejected_so_single_assignment_holds() {
    // Yayılımın dayandığı özellik: aynı adla ikinci `let` hatadır.
    assert_eq!(
        codes("let n = 1;\nlet n = 9;\ndut.addr = n;"),
        vec!["E8506"]
    );
}

// ═══ Tanı içeriği ═════════════════════════════════════════════════

#[test]
fn diagnostic_names_the_bindings_behind_the_value() {
    let found = diags("let n = 7;\nlet m = n + 1;\ndut.addr = m + n;");
    assert_eq!(found.len(), 1, "{found:?}");
    let notes: Vec<&str> = found[0].notes.iter().map(|n| n.text.as_str()).collect();
    assert_eq!(notes.len(), 1, "{found:?}");
    assert!(notes[0].contains("m = 8"), "{notes:?}");
    assert!(notes[0].contains("n = 7"), "{notes:?}");
}

#[test]
fn literal_value_gets_no_binding_note() {
    let found = diags("dut.addr = 8;");
    assert_eq!(found.len(), 1);
    assert!(found[0].notes.is_empty(), "{found:?}");
}

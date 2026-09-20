//! Test bloğunda port genişliği denetimi (ADR-0059, E8512): sabit değer
//! derleme zamanında yakalanır, hesaplanmış değer çalışma zamanına kalır.

use volt_ast::SourceFile;
use volt_hir::{check_tests, const_test_value, test_port_width, ScalarKind};

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
const WIDE : u32 = 12

type Word = u16

module Dut {
    in  clk   : clock
    in  addr  : u3
    in  en    : bool
    in  sv    : i8
    in  wide  : uint<WIDE>
    in  word  : Word
    in  full  : u64
    in  lanes : [u8; 4]
    out data  : u8
    out sdata : i8
    reg r : u8 = 0
    on clk { if en { r <= r + 1 } }
    data = r
    sdata = sv
}
";

fn codes(body: &str) -> Vec<String> {
    let src = format!("{DUT}\ntest \"t\" {{\n    let dut = Dut {{ }};\n{body}\n}}\n");
    let ast = parse(&src);
    check_tests(&[&ast], &ast, false)
        .iter()
        .map(|d| d.code.as_str().to_string())
        .collect()
}

#[test]
fn constant_at_the_port_maximum_is_clean() {
    assert!(codes("dut.addr = 7;").is_empty());
}

#[test]
fn constant_past_the_port_maximum_is_e8512() {
    assert_eq!(codes("dut.addr = 8;"), vec!["E8512"]);
}

#[test]
fn bool_port_rejects_two() {
    assert!(codes("dut.en = true;\ndut.en = 1;").is_empty());
    assert_eq!(codes("dut.en = 2;"), vec!["E8512"]);
}

#[test]
fn constant_expression_is_folded_before_the_check() {
    assert!(codes("dut.addr = 3 + 4;").is_empty());
    assert_eq!(codes("dut.addr = 4 + 4;"), vec!["E8512"]);
    assert_eq!(codes("dut.addr = 1 << 3;"), vec!["E8512"]);
    assert!(codes("dut.addr = 0xFF & 7;").is_empty());
}

#[test]
fn signed_port_takes_bit_patterns_and_in_range_negatives() {
    assert!(codes("dut.sv = 127;\ndut.sv = 255;\ndut.sv = 0 - 1;\ndut.sv = 0 - 128;").is_empty());
    assert_eq!(codes("dut.sv = 256;"), vec!["E8512"]);
    assert_eq!(codes("dut.sv = 0 - 129;"), vec!["E8512"]);
}

#[test]
fn unsigned_port_rejects_a_negative_number() {
    assert_eq!(codes("dut.addr = 0 - 1;"), vec!["E8512"]);
}

#[test]
fn width_given_by_a_const_is_resolved() {
    assert!(codes("dut.wide = 4095;").is_empty());
    assert_eq!(codes("dut.wide = 4096;"), vec!["E8512"]);
}

#[test]
fn sixty_four_bit_port_takes_any_value() {
    assert!(codes("dut.full = 0xFFFFFFFFFFFFFFFF;\ndut.full = 0 - 1;").is_empty());
}

#[test]
fn packed_array_port_is_checked_as_one_vector() {
    assert!(codes("dut.lanes = 0xFFFFFFFF;").is_empty());
    assert_eq!(codes("dut.lanes = 0x100000000;"), vec!["E8512"]);
}

#[test]
fn unresolved_width_is_left_to_the_testbench() {
    // Tip takma adı burada çözülmez: derleme zamanı tanısı yok.
    assert!(codes("dut.word = 0x12345;").is_empty());
}

#[test]
fn computed_value_is_left_to_the_testbench() {
    assert!(codes("for i in 0..16 {\n    dut.addr = i;\n}").is_empty());
    assert!(codes("let n = 8;\ndut.addr = n;").is_empty());
}

#[test]
fn division_by_zero_is_not_a_constant() {
    assert!(codes("dut.addr = 8 / 0;").is_empty());
}

#[test]
fn assert_against_an_unreachable_constant_is_e8512() {
    assert_eq!(codes("assert_eq(dut.data, 300);"), vec!["E8512"]);
    assert_eq!(codes("assert_ne(300, dut.data);"), vec!["E8512"]);
    assert!(codes("assert_eq(dut.data, 255);").is_empty());
}

#[test]
fn signed_output_reads_as_a_bit_pattern() {
    assert!(codes("assert_eq(dut.sdata, 0xFF);").is_empty());
    assert_eq!(codes("assert_eq(dut.sdata, 0 - 1);"), vec!["E8512"]);
}

#[test]
fn diagnostic_points_at_the_value_and_names_the_port() {
    let src = format!("{DUT}\ntest \"t\" {{\n    let dut = Dut {{ }};\n    dut.addr = 8;\n}}\n");
    let ast = parse(&src);
    let diags = check_tests(&[&ast], &ast, false);
    let [diag] = diags.as_slice() else {
        panic!("tek tanı bekleniyor: {diags:?}");
    };
    let primary = diag.primary_span().expect("birincil konum");
    assert_eq!(
        &src[primary.span.start as usize..primary.span.end as usize],
        "8"
    );
    assert!(primary.label.contains("value 8"), "{}", primary.label);
    let secondary = diag
        .spans
        .iter()
        .find(|s| !s.primary)
        .expect("ikincil konum");
    assert!(
        secondary.label.contains("port 'addr' is u3 (max 7)"),
        "{}",
        secondary.label
    );
}

#[test]
fn port_in_a_sibling_file_is_checked_too() {
    let lib = parse(DUT);
    let test = parse("test \"t\" {\n    let dut = Dut { };\n    dut.addr = 9;\n}\n");
    let diags = check_tests(&[&test, &lib], &test, false);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_str(), "E8512");
}

#[test]
fn test_port_width_reports_bits_and_sign() {
    let ast = parse(DUT);
    let width = |port| test_port_width(&[&ast], "Dut", port);
    let addr = width("addr").expect("u3");
    assert_eq!((addr.bits, addr.kind), (3, ScalarKind::UInt));
    let sv = width("sv").expect("i8");
    assert_eq!((sv.bits, sv.is_signed()), (8, true));
    assert_eq!(width("wide").expect("uint<WIDE>").bits, 12);
    assert_eq!(width("lanes").expect("[u8; 4]").bits, 32);
    assert!(width("word").is_none(), "takma ad çözülmez");
    assert!(width("nope").is_none());
    assert!(test_port_width(&[&ast], "Nope", "addr").is_none());
}

#[test]
fn const_test_value_matches_the_generated_cpp_semantics() {
    let value = |expr: &str| {
        let src =
            format!("{DUT}\ntest \"t\" {{\n    let dut = Dut {{ }};\n    dut.full = {expr};\n}}\n");
        let ast = parse(&src);
        let found = ast
            .items
            .iter()
            .find_map(|i| match &ast.items_arena[*i].kind {
                volt_ast::ItemKind::Test(t) => t.stmts.iter().find_map(|s| match s {
                    volt_ast::TestStmt::SetPort { value, .. } => Some(const_test_value(value)),
                    _ => None,
                }),
                _ => None,
            });
        found.expect("port ataması")
    };
    assert_eq!(value("0 - 1"), Some(u64::MAX));
    assert_eq!(value("1 << 64"), Some(0), "64 ve üstü kaydırma 0 verir");
    assert_eq!(value("7 % 0"), None);
    assert_eq!(value("!0 + (3 < 4)"), Some(2));
    assert_eq!(value("dut.data + 1"), None);
}

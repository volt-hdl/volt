//! Hiçbir nitelik sessizce atılmaz (ADR-0098): AST'ye bağlanmayan ya da
//! bağlandığı düğümde hiçbir geçidin okumadığı nitelik W0024 alır;
//! uygulanmayan alan anahtarları (`reset_cycles`, `reset_sequence`) E0003.
//!
//! Önceden `@budget(lut = 10) invariant: ...` gibi kontrat önündeki
//! nitelikler, gövdenin kapanış `}`'inden önceki nitelikler ve `use`
//! önündeki nitelikler ayrıştırılıp tanısız atılıyordu.

use volt_span::FileId;
use volt_syntax::parser::{parse, ParseResult};

fn p(src: &str) -> ParseResult {
    parse(FileId(0), src)
}

fn w0024(result: &ParseResult) -> Vec<&volt_diagnostics::Diagnostic> {
    result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "W0024")
        .collect()
}

fn assert_one_w0024(src: &str, needle: &str) {
    let result = p(src);
    let diags = w0024(&result);
    assert_eq!(diags.len(), 1, "{:?}\n{src}", result.error_codes());
    let labels: Vec<&str> = diags[0].spans.iter().map(|s| s.label.as_str()).collect();
    let text = format!(
        "{} | {:?} | {} | {:?}",
        diags[0].message,
        labels,
        diags[0].help.as_deref().unwrap_or(""),
        diags[0].notes
    );
    assert!(text.contains(needle), "'{needle}' yok: {text}");
}

fn assert_no_w0024(src: &str) {
    let result = p(src);
    assert!(
        w0024(&result).is_empty(),
        "{:?}\n{src}",
        result.error_codes()
    );
}

// ═══ Bağlanmayan nitelikler ════════════════════════════════════════

#[test]
fn attribute_before_a_contract_is_reported() {
    assert_one_w0024(
        "module M {\n in x : u8,\n @budget(lut = 10) invariant: x < 9\n}\n",
        "contract",
    );
}

#[test]
fn attribute_before_a_pipeline_contract_or_stage_is_reported() {
    let src = "pipeline(2) P {\n in clk : clock\n in x : u8\n out y : u8,\n \
               @no_auto_contracts assert: x < 9\n \
               @strict_timing stage A { let a : u8 = x }\n \
               stage B { let b : u8 = a }\n y = stage(B).b\n}\n";
    let result = p(src);
    assert_eq!(w0024(&result).len(), 2, "{:?}", result.error_codes());
}

#[test]
fn attribute_before_the_closing_brace_is_reported() {
    assert_one_w0024(
        "module M {\n in x : u8\n out y : u8\n y = x\n @no_auto_contracts\n}\n",
        "nothing follows",
    );
}

#[test]
fn attribute_before_use_or_at_end_of_file_is_reported() {
    let result = p("@strict_timing use lib::X;\nmodule M { }\n@mmio\n");
    assert_eq!(w0024(&result).len(), 2, "{:?}", result.error_codes());
}

#[test]
fn attribute_before_a_broken_struct_field_is_reported() {
    let result = p("struct S {\n a : u8,\n @allow(unenforced)\n}\n");
    assert_eq!(w0024(&result).len(), 1, "{:?}", result.error_codes());
}

/// Bilinmeyen nitelik zaten W0020 alır ("yok sayılır"); ikinci tanı yok.
#[test]
fn unknown_attribute_is_not_reported_twice() {
    let result = p("module M {\n in x : u8,\n @bilinmeyen invariant: x < 9\n}\n");
    assert!(w0024(&result).is_empty(), "{:?}", result.error_codes());
    assert!(result.error_codes().contains(&"W0020"));
}

// ═══ Yerinde olmayan nitelikler ═══════════════════════════════════

#[test]
fn module_attribute_on_a_port_is_reported() {
    assert_one_w0024(
        "module M {\n @strict_timing in x : u8\n out y : u8\n y = x\n}\n",
        "@strict_timing",
    );
}

#[test]
fn mmio_attribute_on_a_struct_is_reported() {
    assert_one_w0024("@mmio(base = 0) struct S { a : u8 }\n", "struct");
}

#[test]
fn auto_contract_opt_out_on_a_wire_is_reported() {
    assert_one_w0024(
        "module M {\n in x : u8\n out y : u8,\n @no_auto_contracts wire w : u8\n w = x\n y = w\n}\n",
        "reg",
    );
}

#[test]
fn standalone_offset_is_reported_with_the_reg_form() {
    assert_one_w0024(
        "module M {\n in clk : clock\n out y : u8,\n @offset(4) reg r : u8 = 0\n \
         on clk { r <= r }\n y = r\n}\n",
        "@reg(offset",
    );
}

#[test]
fn register_field_attribute_on_a_port_is_reported() {
    assert_one_w0024(
        "module M {\n @w1c in x : u8\n out y : u8\n y = x\n}\n",
        "field",
    );
}

#[test]
fn reg_attribute_on_a_plain_reg_declaration_is_reported() {
    assert_one_w0024(
        "module M {\n in clk : clock\n out y : u8,\n @reg(offset = 0) reg r : u8 = 0\n \
         on clk { r <= r }\n y = r\n}\n",
        "@reg",
    );
}

#[test]
fn timing_attribute_on_a_struct_field_is_reported() {
    assert_one_w0024("struct S {\n @false_path a : u8,\n}\n", "struct field");
}

// ═══ Doğru yerler: tanı yok ═══════════════════════════════════════

#[test]
fn attributes_where_a_pass_reads_them_are_silent() {
    assert_no_w0024(
        "@strict_timing @no_auto_contracts @no_protocol_check @allow(unenforced)\n\
         module M {\n in clk : clock,\n @no_protocol_check @allow(unenforced) in x : u8\n \
         out y : u8,\n @no_auto_contracts @false_path(from = x) reg r : u8 = 0\n \
         on clk { r <= x }\n y = r\n}\n",
    );
    assert_no_w0024(
        "@source(\"x.sv\") @allow(unenforced) extern module X {\n @allow(unenforced) in a : u8\n}\n",
    );
    assert_no_w0024("@timing(clk = 100.mhz) module T {\n in clk : clock\n}\n");
    assert_no_w0024("struct S {\n @allow(unenforced) a : u8,\n}\n");
}

/// Kendi geçidi yanlış yeri zaten raporlayan nitelikler (E0009 / E0017
/// / W0021) ikinci kez uyarılmaz.
#[test]
fn attributes_whose_pass_reports_the_position_are_not_reported_twice() {
    // @source modülde: extern_source E0009 verir.
    assert_no_w0024("@source(\"x.sv\") module M { }\n");
    // @timing reg üstünde: constraints E0017 verir.
    assert_no_w0024("module M {\n in clk : clock,\n @timing reg r : u8 = 0\n}\n");
    // Uygulanmayan nitelik her yerde W0021 alır.
    assert_no_w0024("module M {\n @budget(lut = 1) in x : u8\n}\n");
}

#[test]
fn mmio_module_attributes_are_silent() {
    assert_no_w0024(
        "@mmio(base = 0x4000_0000, bus = AXI4Lite)\nmodule Regs {\n in clk : clock,\n \
         @reg(offset = 0x00, access = ReadWrite) control : {\n enable : bool,\n \
         @w1c done : bool,\n @reserved : bits<30>\n }\n}\n",
    );
}

// ═══ Uygulanmayan alan anahtarları (E0003) ════════════════════════

#[test]
fn reset_cycles_and_reset_sequence_are_not_supported_yet() {
    let result = p(
        "domain D {\n clock = posedge,\n reset = sync active_high,\n \
                    reset_cycles = 4,\n reset_sequence = 1\n}\n",
    );
    let e0003: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E0003")
        .collect();
    assert_eq!(e0003.len(), 2, "{:?}", result.error_codes());
    assert!(
        e0003[0].message.contains("reset_cycles"),
        "{}",
        e0003[0].message
    );
    assert!(
        e0003[1].message.contains("reset_sequence"),
        "{}",
        e0003[1].message
    );
}

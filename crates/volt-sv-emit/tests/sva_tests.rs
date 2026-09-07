//! SVA üretimi testleri (F4a ADIM 2-3).
//!
//! Kontratlar SystemVerilog assertion'larına çevrilir: invariant/ensures
//! /assert → assert property, requires/assume → assume property,
//! cover → cover property. Varsayılan çıktı ayrı .sva dosyası + bind;
//! --sva=inline modunda SV modül gövdesine gömülür.

use volt_span::FileId;
use volt_sv_emit::{emit, emit_full, EmitOutput, SvaMode};

fn full(src: &str, mode: SvaMode) -> EmitOutput {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    let out = emit_full(&parsed.ast, "test.volt", src, mode);
    assert!(
        !out.diagnostics
            .iter()
            .any(|d| d.severity == volt_diagnostics::Severity::Error),
        "emit hatasız olmalı: {:?}",
        out.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    out
}

/// Tek kontratlı tipik modül: tek saat, örtük domain (posedge + sync rst).
fn uart(contracts: &str) -> String {
    format!(
        "module Uart {{\n    in  clk   : clock\n    in  speed : u8\n    \
         in  start : bool\n    out busy  : bool\n\n{contracts}\n    \
         reg busy_r : bool = false\n\n    on clk {{\n        \
         busy_r <= start\n    }}\n\n    busy = busy_r\n}}\n"
    )
}

fn single_sva(src: &str) -> String {
    let out = full(src, SvaMode::Separate);
    assert_eq!(out.sva_files.len(), 1, "tek modül tek .sva üretmeli");
    out.sva_files[0].content.clone()
}

// ═══ Kontrat türü → SVA yapısı ════════════════════════════════════

#[test]
fn invariant_becomes_assert_property() {
    let sva = single_sva(&uart("    invariant: !(start && busy)\n"));
    assert!(sva.contains("property inv_0;"), "{sva}");
    assert!(sva.contains("assert property (inv_0);"), "{sva}");
    assert!(sva.contains("!(start && busy)"), "{sva}");
}

#[test]
fn requires_becomes_assume_property() {
    let sva = single_sva(&uart("    requires: speed <= 2\n"));
    assert!(sva.contains("property req_0;"), "{sva}");
    assert!(sva.contains("assume property (req_0);"), "{sva}");
}

#[test]
fn cover_becomes_cover_property() {
    let sva = single_sva(&uart("    cover: speed == 2 && start\n"));
    assert!(sva.contains("property cov_0;"), "{sva}");
    assert!(sva.contains("cover property (cov_0);"), "{sva}");
}

#[test]
fn ensures_becomes_assert_with_implication_sugar() {
    // Volt'ta '->' yok; üst düzey '!a || b' deseni 'a |-> b' olur.
    let sva = single_sva(&uart("    ensures: !start || busy\n"));
    assert!(sva.contains("property ens_0;"), "{sva}");
    assert!(sva.contains("assert property (ens_0);"), "{sva}");
    assert!(sva.contains("start |-> busy"), "{sva}");
}

#[test]
fn assume_contract_becomes_assume_property() {
    let sva = single_sva(&uart("    assume: speed == 0\n"));
    assert!(sva.contains("property asm_0;"), "{sva}");
    assert!(sva.contains("assume property (asm_0);"), "{sva}");
}

#[test]
fn assert_contract_becomes_assert_property() {
    let sva = single_sva(&uart("    assert: start || !busy\n"));
    assert!(sva.contains("property ast_0;"), "{sva}");
    assert!(sva.contains("assert property (ast_0);"), "{sva}");
}

#[test]
fn same_kind_contracts_get_sequential_names() {
    let sva = single_sva(&uart(
        "    invariant: !(start && busy)\n    invariant: !(busy && speed == 0)\n",
    ));
    assert!(sva.contains("property inv_0;"), "{sva}");
    assert!(sva.contains("property inv_1;"), "{sva}");
}

// ═══ Saat, reset ve kaynak satırı ═════════════════════════════════

#[test]
fn clock_and_reset_come_from_implicit_domain() {
    let sva = single_sva(&uart("    invariant: !(start && busy)\n"));
    assert!(sva.contains("@(posedge clk) disable iff (rst)"), "{sva}");
}

#[test]
fn source_line_comment_is_mandatory() {
    // uart() şablonunda kontrat satırı 7'dedir.
    let sva = single_sva(&uart("    invariant: !(start && busy)\n"));
    assert!(sva.contains("// invariant from test.volt:7"), "{sva}");
}

#[test]
fn active_low_async_reset_becomes_disable_iff_not_rst_n() {
    let src = "domain Sys { clock = posedge, reset = async active_low }\n\n\
               module M {\n    in  clk : clock @Sys\n    in  a : bool\n    out q : bool\n\n    \
               invariant: !(a && q)\n\n    q = a\n}\n";
    let sva = single_sva(src);
    assert!(sva.contains("disable iff (!rst_n)"), "{sva}");
}

#[test]
fn no_reset_domain_omits_disable_iff() {
    let src = "domain Free { clock = posedge, reset = none }\n\n\
               module M {\n    in  clk : clock @Free\n    in  a : bool\n    out q : bool\n\n    \
               invariant: !(a && q)\n\n    q = a\n}\n";
    let sva = single_sva(src);
    assert!(!sva.contains("disable iff"), "{sva}");
    assert!(sva.contains("@(posedge clk)"), "{sva}");
}

#[test]
fn negedge_domain_clocks_properties_on_negedge() {
    let src = "domain Neg { clock = negedge, reset = sync active_high }\n\n\
               module M {\n    in  clk : clock @Neg\n    in  a : bool\n    out q : bool\n\n    \
               invariant: !(a && q)\n\n    q = a\n}\n";
    let sva = single_sva(src);
    assert!(sva.contains("@(negedge clk)"), "{sva}");
}

// ═══ Ayrı dosya biçimi (bind) ═════════════════════════════════════

#[test]
fn separate_file_wraps_checker_module_with_bind() {
    let out = full(
        &uart("    invariant: !(start && busy)\n"),
        SvaMode::Separate,
    );
    let file = &out.sva_files[0];
    assert_eq!(file.module_name, "Uart");
    assert_eq!(file.checker_name, "uart_sva");
    assert!(
        file.content.contains("module uart_sva ("),
        "{}",
        file.content
    );
    assert!(
        file.content.contains("bind Uart uart_sva sva_inst (.*);"),
        "{}",
        file.content
    );
}

#[test]
fn checker_ports_carry_referenced_signal_widths() {
    let sva = single_sva(&uart("    requires: speed <= 2\n"));
    assert!(sva.contains("input logic [7:0] speed"), "{sva}");
    assert!(sva.contains("input logic"), "{sva}");
}

#[test]
fn checker_ports_include_registers_used_by_invariant() {
    let sva = single_sva(&uart("    invariant: !(busy_r && start)\n"));
    assert!(sva.contains("busy_r"), "{sva}");
    assert!(sva.contains("bind Uart"), "{sva}");
}

#[test]
fn module_without_contracts_produces_no_sva_file() {
    let out = full(
        "module Plain {\n    in  clk : clock\n    in  a : bool\n    out q : bool\n\n    q = a\n}\n",
        SvaMode::Separate,
    );
    assert!(out.sva_files.is_empty());
}

#[test]
fn clockless_module_contracts_are_skipped() {
    // Formel araçlar saat ister; saatsiz modülde SVA üretilmez.
    let out = full(
        "module Comb {\n    in  a : bool\n    out q : bool\n\n    \
         invariant: !(a && q)\n\n    q = a\n}\n",
        SvaMode::Separate,
    );
    assert!(out.sva_files.is_empty());
}

// ═══ Inline mod ═══════════════════════════════════════════════════

#[test]
fn inline_mode_embeds_properties_in_module_body() {
    let out = full(&uart("    invariant: !(start && busy)\n"), SvaMode::Inline);
    assert!(out.sva_files.is_empty(), "inline modda ayrı dosya yok");
    assert!(out.sv.contains("property inv_0;"), "{}", out.sv);
    assert!(out.sv.contains("assert property (inv_0);"), "{}", out.sv);
    assert!(
        !out.sv.contains("bind "),
        "inline modda bind yok: {}",
        out.sv
    );
    // Özellik bloğu modülün İÇİNDE olmalı.
    let prop = out.sv.find("property inv_0;").unwrap();
    let end = out.sv.find("endmodule").unwrap();
    assert!(prop < end, "SVA bloğu endmodule'den önce olmalı");
}

#[test]
fn plain_emit_output_is_unchanged() {
    let parsed = volt_syntax::parser::parse(FileId(0), &uart("    invariant: !(start && busy)\n"));
    let result = emit(&parsed.ast, "test.volt");
    assert!(!result.sv.contains("property"), "emit() SVA içermemeli");
    assert!(!result.sv.contains("bind "), "emit() bind içermemeli");
}

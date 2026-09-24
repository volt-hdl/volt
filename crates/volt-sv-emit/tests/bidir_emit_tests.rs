//! Çift yönlü port SV üretimi — ADR-0051, sv-mapping.md §17: `inout wire`
//! port, `assign p = enable ? value : 'z` üç durumlu tampon, `tri1`
//! veri yolu telleri, formal (Immediate) modda anyseq dış aygıt modeli.

use volt_span::FileId;
use volt_sv_emit::{emit, emit_full, SvaMode};

fn parse(src: &str) -> volt_ast::SourceFile {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    parsed.ast
}

fn sv(src: &str) -> String {
    let ast = parse(src);
    let result = emit(&ast, "test.volt");
    assert!(
        !result.has_errors(),
        "emit hatasız olmalı: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    result.sv
}

fn formal_sv(src: &str) -> String {
    let ast = parse(src);
    let out = emit_full(&ast, "test.volt", src, SvaMode::Immediate);
    assert!(
        !out.diagnostics.iter().any(|d| !d.code.is_warning()),
        "emit hatasız olmalı: {:?}",
        out.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    out.sv
}

const PAD: &str = "module Pad {\n    in clk : clock\n    in en : bool\n    opendrain sda : bool\n    out q : bool\n    on clk {\n        if en { sda.drive_low() } else { sda.release() }\n    }\n    q = sda.read()\n}\n";

const SRAM: &str = "module Sram {\n    in clk : clock\n    in we : bool\n    in v : bits<8>\n    inout dq : bits<8>\n    out q : bits<8>\n    on clk {\n        if we { dq.drive(v) } else { dq.release() }\n    }\n    q = dq.read()\n}\n";

#[test]
fn opendrain_port_is_an_inout_wire() {
    let sv = sv(PAD);
    assert!(sv.contains("    inout  wire  sda,"), "{sv}");
    assert!(!sv.contains("inout  logic"), "{sv}");
}

#[test]
fn opendrain_driver_is_a_tristate_assign_to_zero() {
    let sv = sv(PAD);
    assert!(
        sv.contains("assign sda = sda_drive_low ? 1'b0 : 1'bz;"),
        "{sv}"
    );
    assert!(sv.contains("logic sda_drive_low;"), "{sv}");
}

#[test]
fn inout_vector_port_and_driver_replicate_z() {
    let sv = sv(SRAM);
    assert!(sv.contains("    inout  wire [7:0]  dq,"), "{sv}");
    assert!(
        sv.contains("assign dq = dq_oe ? dq_out : {8{1'bz}};"),
        "{sv}"
    );
    assert!(sv.contains("logic [7:0] dq_out;"), "{sv}");
}

#[test]
fn drive_registers_reset_to_released_and_zero() {
    let sv = sv(SRAM);
    assert!(sv.contains("dq_oe <= 1'b0;"), "{sv}");
    assert!(sv.contains("dq_out <= 8'd0;"), "{sv}");
    assert!(sv.contains("dq_oe <= 1'b1;"), "{sv}");
    assert!(sv.contains("dq_out <= v;"), "{sv}");
}

#[test]
fn read_call_reads_the_net_directly() {
    let sv = sv(PAD);
    assert!(sv.contains("assign q = sda;"), "{sv}");
}

#[test]
fn read_only_pad_gets_no_driver() {
    let sv = sv("module L {\n    in clk : clock\n    inout pin : bool\n    out q : bool\n    q = pin.read()\n}\n");
    assert!(!sv.contains("assign pin ="), "{sv}");
    assert!(sv.contains("inout  wire  pin"), "{sv}");
}

#[test]
fn observed_but_undriven_pad_is_a_constant_enable() {
    let sv = sv("module O {\n    in clk : clock\n    opendrain sda : bool\n    out held : bool\n    held = sda.driving\n}\n");
    assert!(sv.contains("wire sda_drive_low = 1'b0;"), "{sv}");
    assert!(
        sv.contains("assign sda = sda_drive_low ? 1'b0 : 1'bz;"),
        "{sv}"
    );
    assert!(sv.contains("assign held = sda_drive_low;"), "{sv}");
}

#[test]
fn contract_released_is_the_negated_enable() {
    let ast = parse("module C {\n    in clk : clock\n    in en : bool\n    opendrain sda : bool\n    invariant: !en_r -> sda.released\n    reg en_r : bool = false\n    on clk {\n        en_r <= en\n        if en { sda.drive_low() } else { sda.release() }\n    }\n}\n");
    let out = emit_full(&ast, "c.volt", "", SvaMode::Inline);
    assert!(out.sv.contains("!sda_drive_low"), "{}", out.sv);
}

#[test]
fn parent_wire_bound_to_opendrain_instances_is_tri1() {
    let src = format!(
        "{PAD}module Bus {{\n    in clk : clock\n    in a : bool\n    in b : bool\n    out line : bool\n    wire sda_bus : bool\n    let pa = Pad {{ clk, en: a, sda: sda_bus }}\n    let pb = Pad {{ clk, en: b, sda: sda_bus }}\n    line = sda_bus\n}}\n"
    );
    let sv = sv(&src);
    assert!(sv.contains("    tri1 sda_bus;"), "{sv}");
    assert!(nospace(&sv).contains(".sda(sda_bus)"), "{sv}");
    // Çift yönlü port çıkış teli değildir: `pa_sda` üretilmez.
    assert!(!sv.contains("pa_sda"), "{sv}");
}

/// Hizalama boşluklarından bağımsız karşılaştırma için tüm boşlukları atar.
fn nospace(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn parent_wire_bound_to_inout_instance_is_a_plain_wire() {
    let src = format!(
        "{SRAM}module Board {{\n    in clk : clock\n    in we : bool\n    in v : bits<8>\n    out q : bits<8>\n    wire dq_bus : bits<8>\n    let s = Sram {{ clk, we, v, dq: dq_bus }}\n    q = s.q\n}}\n"
    );
    let sv = sv(&src);
    assert!(sv.contains("    wire [7:0] dq_bus;"), "{sv}");
    assert!(sv.contains(".dq (dq_bus)"), "{sv}");
}

#[test]
fn unbound_bidirectional_instance_port_is_e4011() {
    let src = format!(
        "{PAD}module Top {{\n    in clk : clock\n    in a : bool\n    out q : bool\n    let pa = Pad {{ clk, en: a }}\n    q = pa.q\n}}\n"
    );
    let ast = parse(&src);
    let result = emit(&ast, "test.volt");
    let codes: Vec<_> = result.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"E4011"), "{codes:?}");
}

#[test]
fn formal_output_models_the_external_device_with_anyseq() {
    let sv = formal_sv(PAD);
    assert!(sv.contains("(* anyseq *) logic sda_ext;"), "{sv}");
    assert!(
        sv.contains("assign sda = sda_drive_low ? 1'b0 : sda_ext;"),
        "{sv}"
    );
    assert!(!sv.contains("1'bz"), "{sv}");
}

#[test]
fn formal_output_declares_bus_wires_without_tri1() {
    let src = format!(
        "{PAD}module Bus {{\n    in clk : clock\n    in a : bool\n    out line : bool\n    wire sda_bus : bool\n    let pa = Pad {{ clk, en: a, sda: sda_bus }}\n    line = sda_bus\n}}\n"
    );
    let sv = formal_sv(&src);
    assert!(sv.contains("    wire sda_bus;"), "{sv}");
    assert!(!sv.contains("tri1"), "{sv}");
}

#[test]
fn port_order_puts_bidirectional_pads_between_inputs_and_outputs() {
    let sv = sv("module P {\n    in clk : clock\n    in a : bool\n    opendrain sda : bool\n    inout dq : u8\n    out q : bool\n    on clk { sda.release() }\n    q = a\n}\n");
    // §1 sırası: in → inout → opendrain → out (bildirim sırası değil).
    let flat = nospace(&sv);
    let a = flat.find("inputlogica,").expect("a");
    let dq = flat.find("inoutwire[7:0]dq,").expect("dq");
    let sda = flat.find("inoutwiresda,").expect("sda");
    let q = flat.find("outputlogicq").expect("q");
    assert!(a < dq && dq < sda && sda < q, "{sv}");
}

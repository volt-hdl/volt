//! `extern module` örneklemesinin SV eşlemesi (ADR-0071): bildirilen
//! portlar, bildirim sırasıyla, adlandırılmış bağlantı; örtük reset portu
//! yok; extern'in kendisi için SV modülü üretilmez.

use volt_span::FileId;
use volt_sv_emit::{emit, EmitResult};

fn run(src: &str) -> EmitResult {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    emit(&parsed.ast, "test.volt")
}

fn codes(result: &EmitResult) -> Vec<&str> {
    result.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn sv(src: &str) -> String {
    let result = run(src);
    assert!(
        !result.has_errors(),
        "emit hatasız olmalı: {:?}",
        codes(&result)
    );
    result.sv
}

/// `Ext e (` ile başlayan örnekleme bloğu.
fn instance_block<'a>(sv: &'a str, header: &str) -> &'a str {
    let start = sv
        .find(header)
        .unwrap_or_else(|| panic!("{header} yok:\n{sv}"));
    let end = start + sv[start..].find(");").expect("örnekleme kapanmalı");
    &sv[start..end]
}

const SYNC_DOMAIN_EXTERN: &str = "
domain Sys {
    clock = posedge
    reset = sync active_high
}
extern module Ext {
    in  clk : clock @Sys
    in  d   : u8    @Sys
    out q   : u8    @Sys
}
module M {
    in  clk : clock @Sys
    in  x   : u8    @Sys
    out y   : u8    @Sys
    let e = Ext { clk, d: x }
    y = e.q
}
";

#[test]
fn extern_instance_uses_declared_ports_in_declaration_order() {
    let sv = sv(SYNC_DOMAIN_EXTERN);
    let block = instance_block(&sv, "Ext e (");
    let ports: Vec<&str> = block
        .lines()
        .filter_map(|l| l.trim().strip_prefix('.'))
        .map(|l| l.split('(').next().unwrap().trim())
        .collect();
    assert_eq!(ports, ["clk", "d", "q"], "{block}");
    assert!(block.contains(".d  (x)"), "{block}");
    assert!(block.contains(".q  (e_q)"), "{block}");
}

#[test]
fn extern_instance_gets_no_implicit_reset_port() {
    // Volt modülü aynı alanda `.rst(rst)` alırdı; extern'in SV arayüzü
    // bildirilen portlardır.
    let sv = sv(SYNC_DOMAIN_EXTERN);
    let block = instance_block(&sv, "Ext e (");
    assert!(!block.contains("rst"), "{block}");
}

#[test]
fn extern_output_is_predeclared_and_readable() {
    let sv = sv(SYNC_DOMAIN_EXTERN);
    assert!(sv.contains("logic [7:0] e_q;"), "{sv}");
    assert!(sv.contains("assign y = e_q;"), "{sv}");
}

#[test]
fn extern_declaration_emits_no_sv_module() {
    let sv = sv(SYNC_DOMAIN_EXTERN);
    assert!(!sv.contains("module Ext"), "{sv}");
    assert!(sv.contains("module M"), "{sv}");
}

#[test]
fn extern_bidir_raw_reset_and_array_ports_are_bound_by_name() {
    let sv = sv("
extern module ExtIo {
    in    clk   : clock
    in    rst_n : reset
    inout sda   : bool
    in    d     : [u4; 2]
    out   q     : i8
}
module Top {
    in  clk   : clock
    in  rst_n : reset
    in  d     : [u4; 2]
    out q     : i8
    wire sda_bus : bool
    let e = ExtIo { clk, rst_n, sda: sda_bus, d }
    q = e.q
}
");
    let block = instance_block(&sv, "ExtIo e (");
    for conn in [
        ".clk  (clk)",
        ".rst_n(rst_n)",
        ".sda  (sda_bus)",
        ".d    (d)",
        ".q    (e_q)",
    ] {
        assert!(block.contains(conn), "{conn} yok:\n{block}");
    }
    assert!(sv.contains("logic signed [7:0] e_q;"), "{sv}");
}

#[test]
fn unbound_extern_input_is_e2005() {
    let result = run("
extern module Ext {
    in  clk : clock
    in  d   : u8
    out q   : u8
}
module M {
    in  clk : clock
    out y   : u8
    let e = Ext { clk }
    y = e.q
}
");
    assert_eq!(codes(&result), ["E2005"]);
}

#[test]
fn generic_extern_instance_is_e0003_at_the_instance() {
    // Extern generic'leri monomorfize edilmez (ADR-0047): tek E0003,
    // extern'in port bildiriminde değil örneklemede.
    let result = run("
extern module ExtG<T> {
    in  d : T
    out q : T
}
module M {
    out y : u8
    let x = ExtG<u8> { d: 3 }
    y = x.q
}
");
    assert_eq!(codes(&result), ["E0003"]);
    let msg = &result.diagnostics[0].message;
    assert!(msg.contains("generic extern module 'ExtG'"), "{msg}");
}

#[test]
fn ui_pass_62_emits_both_extern_instances() {
    let src = include_str!("../../../tests/ui/pass/62_extern_domains.volt");
    let sv = sv(src);
    let fifo = instance_block(&sv, "ExtAsyncFifo f (");
    assert!(fifo.contains(".wr_clk  (sys_clk)"), "{fifo}");
    assert!(fifo.contains(".rd_clk  (pix_clk)"), "{fifo}");
    let dly = instance_block(&sv, "ExtDelay dly (");
    assert!(dly.contains(".d  (f_rd_data)"), "{dly}");
    assert!(sv.contains("assign pix_d = dly_q;"), "{sv}");
}

//! Testbench reset'i modülün GERÇEK portlarını sürer (ADR-0098).
//!
//! Önceden ham reset portu olmayan her modülde `apply_reset` körü körüne
//! `dut->rst` yazıyordu. Saat portu olmayan (tamamen kombinasyonel)
//! modülde, `reset = none` alanında ve `active_low` alanında (`rst_n`)
//! böyle bir port yoktur: `volt test` C++ derlemesinde düşüyordu.

use volt_span::FileId;
use volt_sv_emit::{
    collect_sim_ports, emit, find_module, run_testbench_cpp, test_testbench_cpp, SimReset, TbStep,
    TbTest,
};

fn parse(src: &str) -> volt_ast::SourceFile {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    parsed.ast
}

/// Modülün testbench'i ve üretilen SV'nin port bloğu.
fn tb_and_ports(src: &str, module: &str) -> (String, String, Vec<volt_sv_emit::SimPort>) {
    let ast = parse(src);
    let sv = emit(&ast, "test.volt").sv;
    let start = sv.find(&format!("module {module} (")).expect("modül");
    let end = start + sv[start..].find(");").expect("port sonu");
    let ports_block = sv[start..end].to_string();
    let (src, m) = find_module(&[&ast], module).expect("modül bulunmalı");
    let ports = collect_sim_ports(src, m);
    let tests = [TbTest {
        name: "t".into(),
        steps: vec![TbStep::Step(1)],
    }];
    (
        test_testbench_cpp(module, &ports, &tests),
        ports_block,
        ports,
    )
}

/// `apply_reset` gövdesi.
fn reset_body(cpp: &str) -> &str {
    let start = cpp.find("static void apply_reset").expect("apply_reset");
    let end = start + cpp[start..].find("\n}\n").expect("gövde sonu");
    &cpp[start..end]
}

const CLOCKLESS: &str = "module Adder { in a : u8 in b : u8 out sum : u9 let s = a + b sum = s }";

#[test]
fn clockless_module_testbench_drives_no_reset() {
    let (cpp, ports_block, ports) = tb_and_ports(CLOCKLESS, "Adder");
    assert!(!ports_block.contains("rst"), "{ports_block}");
    assert!(ports.iter().all(|p| p.reset.is_none()), "{ports:?}");
    assert!(!cpp.contains("dut->rst"), "{cpp}");
    let body = reset_body(&cpp);
    assert!(!body.contains("run_cycle"), "{body}");
    // Model bir kez oturtulur: yoksa ilk posedge kenar sayılmaz.
    assert!(body.contains("dut->eval();"), "{body}");
}

#[test]
fn reset_none_domain_testbench_drives_no_reset() {
    let src = "domain D { clock = posedge, reset = none }\n\
               module Shift { in clk : clock @D in d : u8 out q : u8 \
               reg r : u8 = 0 on clk { r <= d } q = r }";
    let (cpp, ports_block, _) = tb_and_ports(src, "Shift");
    assert!(!ports_block.contains("rst"), "{ports_block}");
    assert!(!cpp.contains("dut->rst"), "{cpp}");
}

#[test]
fn active_low_domain_testbench_drives_rst_n() {
    let src = "domain Low { clock = posedge, reset = sync active_low }\n\
               module C { in clk : clock @Low out q : u8 \
               reg r : u8 = 5 on clk { r <= r + 1 } q = r }";
    let (cpp, ports_block, ports) = tb_and_ports(src, "C");
    assert!(ports_block.contains("rst_n"), "{ports_block}");
    assert_eq!(
        ports
            .iter()
            .find(|p| p.name == "rst_n")
            .and_then(|p| p.reset),
        Some(SimReset::Auto(volt_ast::ResetPolarity::ActiveLow))
    );
    let body = reset_body(&cpp);
    assert!(body.contains("dut->rst_n = 0;"), "{body}");
    assert!(body.contains("dut->rst_n = 1;"), "{body}");
    assert!(!cpp.contains("dut->rst ="), "{cpp}");
}

/// Varsayılan alan (`rst`, aktif-yüksek): üretilen metin önceki ile aynı —
/// 2 çevrim reset, senkronizör beklemesi yok (test zamanlaması değişmez).
#[test]
fn default_domain_testbench_reset_is_unchanged() {
    let src = "module Cnt { in clk : clock out q : u8 reg r : u8 = 0 \
               on clk { r <= r + 1 } q = r }";
    let (cpp, _, _) = tb_and_ports(src, "Cnt");
    let body = reset_body(&cpp);
    assert_eq!(
        body,
        "static void apply_reset(TOP* dut, VerilatedContext* ctx) {\n    \
         dut->rst = 1;\n    run_cycle(dut, ctx);\n    run_cycle(dut, ctx);\n    \
         dut->rst = 0;"
    );
}

#[test]
fn run_testbench_of_a_clockless_module_drives_no_reset() {
    let ast = parse(CLOCKLESS);
    let (src, m) = find_module(&[&ast], "Adder").expect("modül");
    let ports = collect_sim_ports(src, m);
    let cpp = run_testbench_cpp("Adder", &ports, 4, None);
    assert!(!cpp.contains("dut->rst"), "{cpp}");
}

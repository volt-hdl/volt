//! Ham reset portu ve bırakma senkronizörü (ADR-0065 §1-§2): zincir
//! biçimi, polarite, port sırası, örnekleme bağlantısı, yerleşik
//! primitifler, SVA ve Verilator testbench'i.

use volt_span::FileId;
use volt_sv_emit::{
    collect_sim_ports, emit, emit_full, find_module, run_testbench_cpp, test_testbench_cpp,
    SimReset, SvaMode, TbStep, TbTest,
};

fn parse(src: &str) -> volt_ast::SourceFile {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    parsed.ast
}

fn sv(src: &str) -> String {
    let ast = parse(src);
    let out = emit(&ast, "test.volt");
    assert!(
        !out.has_errors(),
        "{:?}",
        out.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    out.sv
}

/// `module X (` ile `);` arasındaki port listesi.
fn port_block<'a>(sv: &'a str, module: &str) -> &'a str {
    let start = sv.find(&format!("module {module} (")).expect("modül");
    let end = start + sv[start..].find(");").expect("port sonu");
    &sv[start..end]
}

const TWO_CLOCKS: &str = include_str!("../../../tests/ui/pass/83_rdc_raw_reset_two_clocks.volt");
const PER_DOMAIN: &str = include_str!("../../../tests/ui/pass/84_rdc_raw_reset_per_domain.volt");
const HIERARCHY: &str = include_str!("../../../tests/ui/pass/85_rdc_raw_reset_hierarchy.volt");
const SYNC_DOMAINS: &str =
    include_str!("../../../tests/ui/pass/86_rdc_raw_reset_sync_domains.volt");
const ASYNC_FIFO: &str = include_str!("../../../tests/ui/pass/27_async_fifo.volt");

#[test]
fn raw_port_gets_one_two_stage_chain_per_clock() {
    let out = sv(TWO_CLOCKS);
    for clk in ["fast_clk", "slow_clk"] {
        assert!(
            out.contains(&format!(
                "    // reset synchronizer: rst_n -> {clk} (async assert, sync release)\n\
                 \x20   logic rst_sync_{clk}_stage0;\n\
                 \x20   logic rst_sync_{clk}_stage1;\n\
                 \x20   always_ff @(posedge {clk} or negedge rst_n) begin\n\
                 \x20       if (!rst_n) begin\n\
                 \x20           rst_sync_{clk}_stage0 <= 1'b0;\n\
                 \x20           rst_sync_{clk}_stage1 <= 1'b0;\n\
                 \x20       end else begin\n\
                 \x20           rst_sync_{clk}_stage0 <= 1'b1;\n\
                 \x20           rst_sync_{clk}_stage1 <= rst_sync_{clk}_stage0;\n\
                 \x20       end\n    end"
            )),
            "{clk} zinciri:\n{out}"
        );
    }
    assert_eq!(out.matches("// reset synchronizer:").count(), 2);
}

#[test]
fn domain_registers_reset_from_their_own_chain() {
    let out = sv(TWO_CLOCKS);
    assert!(out.contains("always_ff @(posedge fast_clk or negedge rst_sync_fast_clk_stage1) begin"));
    assert!(out.contains("if (!rst_sync_fast_clk_stage1) begin"));
    // sync() aşamaları hedef alanın zinciriyle sıfırlanır.
    assert!(out.contains("always_ff @(posedge slow_clk or negedge rst_sync_slow_clk_stage1) begin"));
    // Ham port yalnız zincirin CLR pinini sürer.
    assert_eq!(out.matches("negedge rst_n)").count(), 2, "{out}");
}

#[test]
fn fed_domains_get_no_automatic_reset_port() {
    let out = sv(TWO_CLOCKS);
    let ports = port_block(&out, "Bridge");
    assert_eq!(ports.matches("rst_n").count(), 1, "{ports}");
}

#[test]
fn raw_port_comes_after_clocks_and_before_data() {
    let out = sv(TWO_CLOCKS);
    let ports = port_block(&out, "Bridge");
    let pos = |s: &str| ports.find(s).unwrap_or_else(|| panic!("{s} yok:\n{ports}"));
    assert!(pos("slow_clk") < pos("rst_n"));
    assert!(pos("rst_n") < pos("fast_in"));
}

#[test]
fn active_high_raw_reset_asserts_to_one() {
    let src = "domain D { clock = posedge, reset = async active_high }\n\
               module M { in clk : clock @D in rst : reset(async, active_high) out q : u8 \
               reg r : u8 = 0 on clk { r <= r + 1 } q = r }";
    let out = sv(src);
    assert!(out.contains("always_ff @(posedge clk or posedge rst) begin\n        if (rst) begin"));
    assert!(out.contains("rst_sync_clk_stage0 <= 1'b1;\n            rst_sync_clk_stage1 <= 1'b1;"));
    assert!(out.contains(
        "rst_sync_clk_stage0 <= 1'b0;\n            rst_sync_clk_stage1 <= rst_sync_clk_stage0;"
    ));
    assert!(out.contains("always_ff @(posedge clk or posedge rst_sync_clk_stage1) begin\n        if (rst_sync_clk_stage1) begin"));
}

#[test]
fn raw_port_without_kind_takes_the_domain_polarity() {
    let src = "domain D { clock = posedge, reset = async active_low }\n\
               module M { in clk : clock @D in r : reset out q : u8 \
               reg x : u8 = 0 on clk { x <= x + 1 } q = x }";
    let out = sv(src);
    assert!(
        out.contains("always_ff @(posedge clk or negedge r) begin\n        if (!r) begin"),
        "{out}"
    );
}

#[test]
fn negedge_sync_domain_chain_and_registers() {
    let out = sv(PER_DOMAIN);
    assert!(
        out.contains("always_ff @(negedge slow_clk or posedge slow_rst) begin"),
        "{out}"
    );
    // Senkron alan: flop'lar yalnız saat kenarında, reset koşulu zincirden.
    assert!(out.contains(
        "always_ff @(negedge slow_clk) begin\n        if (rst_sync_slow_clk_stage1) begin"
    ));
    assert!(out.contains("always_ff @(posedge fast_clk or negedge fast_rst_n) begin"));
}

#[test]
fn raw_sync_reset_replaces_the_shared_rst_port() {
    let out = sv(SYNC_DOMAINS);
    let ports = port_block(&out, "Video");
    assert_eq!(
        ports.matches("rst").count(),
        1,
        "yalnız ham 'rst':\n{ports}"
    );
    assert!(out.contains(
        "always_ff @(posedge sys_clk) begin\n        if (rst_sync_sys_clk_stage1) begin"
    ));
    assert!(out.contains(
        "always_ff @(posedge pix_clk) begin\n        if (rst_sync_pix_clk_stage1) begin"
    ));
}

#[test]
fn unfed_domain_keeps_its_automatic_port() {
    let src = "domain Fast { clock = posedge, reset = async active_low }\n\
               domain Slow { clock = posedge, reset = async active_low }\n\
               module M { in fast_clk : clock @Fast in slow_clk : clock @Slow \
               in ext_rst_n : reset(async, active_low) @Fast out qa : u8 @Fast out qb : u8 @Slow \
               reg(fast_clk) ra : u8 = 0 reg(slow_clk) rb : u8 = 0 \
               on fast_clk { ra <= ra + 1 } on slow_clk { rb <= rb + 1 } qa = ra qb = rb }";
    let out = sv(src);
    let ports = port_block(&out, "M");
    let pos = |s: &str| ports.find(s).unwrap_or_else(|| panic!("{s} yok:\n{ports}"));
    assert!(
        pos("rst_n,") < pos("ext_rst_n"),
        "otomatik port ham porttan önce"
    );
    assert!(out.contains("always_ff @(posedge slow_clk or negedge rst_n) begin"));
    assert!(out.contains("always_ff @(posedge fast_clk or negedge rst_sync_fast_clk_stage1) begin"));
    assert_eq!(out.matches("// reset synchronizer:").count(), 1);
}

#[test]
fn child_automatic_reset_is_driven_by_the_parent_chain() {
    let out = sv(HIERARCHY);
    assert!(out.contains(".rst_n(rst_sync_clk_stage1)"), "{out}");
    // Çocuk kendi otomatik portunu korur, zincir üretmez.
    let child = port_block(&out, "Child");
    assert!(child.contains("rst_n"), "{child}");
    assert_eq!(out.matches("// reset synchronizer:").count(), 1);
}

#[test]
fn child_raw_port_is_connected_from_the_binding() {
    let src = "domain D { clock = posedge, reset = async active_low }\n\
               module C { in clk : clock @D in cr_n : reset(async, active_low) out q : u8 \
               reg r : u8 = 0 on clk { r <= r + 1 } q = r }\n\
               module T { in clk : clock @D in ext_n : reset(async, active_low) @Other out q : u8 \
               let u = C { clk: clk, cr_n: ext_n } q = u.q }";
    let out = sv(src);
    assert!(out.contains(".cr_n(ext_n)"), "{out}");
}

#[test]
fn builtin_async_fifo_sides_reset_from_the_chains() {
    let src = ASYNC_FIFO.replace(
        "    in  din      : u8    @Fast",
        "    in  rst      : reset(sync, active_high)\n    in  din      : u8    @Fast",
    );
    let out = sv(&src);
    assert!(out.contains("if (rst_sync_fast_clk_stage1) begin"), "{out}");
    assert!(out.contains("if (rst_sync_slow_clk_stage1) begin"), "{out}");
    let ports = port_block(&out, "FifoBridge");
    assert_eq!(ports.matches("rst").count(), 1, "{ports}");
}

const CONTRACT: &str = "domain D { clock = posedge, reset = async active_low }\n\
    module M { in clk : clock @D in rst_n : reset(async, active_low) out q : u8 \
    reg r : u8 = 0 on clk { r <= r + 1 } q = r invariant: r <= 255 }";

#[test]
fn inline_sva_is_disabled_by_the_chain_output() {
    let ast = parse(CONTRACT);
    let out = emit_full(&ast, "t.volt", CONTRACT, SvaMode::Inline);
    assert!(
        out.sv.contains("disable iff (!rst_sync_clk_stage1)"),
        "{}",
        out.sv
    );
    assert!(
        out.sv.contains("initial assume (!rst_sync_clk_stage1);"),
        "{}",
        out.sv
    );
}

#[test]
fn separate_sva_binds_the_chain_output() {
    let ast = parse(CONTRACT);
    let out = emit_full(&ast, "t.volt", CONTRACT, SvaMode::Separate);
    let file = out.sva_files.first().expect("sva dosyası");
    assert!(
        file.content.contains("rst_sync_clk_stage1"),
        "{}",
        file.content
    );
}

// ═══ Verilator testbench'i ════════════════════════════════════════

const SIM_SRC: &str = "domain Fast { clock = posedge, reset = async active_low }\n\
    domain Slow { clock = posedge, reset = async active_low }\n\
    module M { in fast_clk : clock @Fast in slow_clk : clock @Slow \
    in ext_rst_n : reset(async, active_low) @Fast in a : u8 @Fast out qa : u8 @Fast out qb : u8 @Slow \
    reg(fast_clk) ra : u8 = 0 reg(slow_clk) rb : u8 = 0 \
    on fast_clk { ra <= a } on slow_clk { rb <= rb + 1 } qa = ra qb = rb }";

#[test]
fn sim_ports_mark_raw_and_remaining_automatic_resets() {
    let ast = parse(SIM_SRC);
    let (src, m) = find_module(&[&ast], "M").expect("M");
    let ports = collect_sim_ports(src, m);
    let reset_of = |n: &str| ports.iter().find(|p| p.name == n).and_then(|p| p.reset);
    assert_eq!(
        reset_of("ext_rst_n"),
        Some(SimReset::Raw(volt_ast::ResetPolarity::ActiveLow))
    );
    assert_eq!(
        reset_of("rst_n"),
        Some(SimReset::Auto(volt_ast::ResetPolarity::ActiveLow))
    );
    assert_eq!(reset_of("a"), None);
}

#[test]
fn testbench_drives_every_reset_and_waits_for_the_chain() {
    let ast = parse(SIM_SRC);
    let (src, m) = find_module(&[&ast], "M").expect("M");
    let ports = collect_sim_ports(src, m);
    let tests = [TbTest {
        name: "t".into(),
        steps: vec![TbStep::Step(1)],
    }];
    let cpp = test_testbench_cpp("M", &ports, &tests);
    let body = cpp
        .split("static void apply_reset")
        .nth(1)
        .and_then(|s| s.split("\n}\n").next())
        .expect("apply_reset");
    assert!(
        body.contains("dut->ext_rst_n = 0;") && body.contains("dut->ext_rst_n = 1;"),
        "{body}"
    );
    assert!(
        body.contains("dut->rst_n = 0;") && body.contains("dut->rst_n = 1;"),
        "{body}"
    );
    assert!(!body.contains("dut->rst ="), "{body}");
    // 2 çevrim reset + 2 çevrim zincir bırakması.
    assert_eq!(body.matches("run_cycle").count(), 4, "{body}");
    // Reset girişleri veri gibi sıfırlanmaz.
    assert!(!cpp.contains("    dut.ext_rst_n = 0;"), "{cpp}");
}

#[test]
fn run_table_does_not_print_reset_inputs() {
    let ast = parse(SIM_SRC);
    let (src, m) = find_module(&[&ast], "M").expect("M");
    let ports = collect_sim_ports(src, m);
    let cpp = run_testbench_cpp("M", &ports, 3, None);
    assert!(!cpp.contains("(unsigned long long)dut.ext_rst_n"), "{cpp}");
    assert!(
        !cpp.contains("dut.ext_rst_n = 1;"),
        "duman uyarımı reset'i sürmez:\n{cpp}"
    );
    assert!(cpp.contains("(unsigned long long)dut.qa"));
}

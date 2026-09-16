//! Zamanlama kısıtı modeli testleri (ADR-0054, constraints/).
//!
//! Kapsam: domain frekansı → periyot, `@timing` biçimleri ve E0017
//! tanıları, alan frekansıyla tutarlılık, asenkron gruplar, üretilen
//! köprüler (sync/sync3/AsyncFifo/HandshakeSync/PulseSync/
//! AsyncDualPortRam), hiyerarşik önek, W0022'nin yalnız istek üzerine
//! üretilmesi ve W0021'in bu üç nitelik için susması.

use volt_diagnostics::Diagnostic;
use volt_hir::{analyze, collect_constraints, ConstraintResult, PathKind, Target};
use volt_syntax::{parse, FileId};

fn parse_clean(src: &str) -> volt_ast::SourceFile {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hataları: {:?}",
        parsed.error_codes()
    );
    parsed.ast
}

fn collect(src: &str) -> ConstraintResult {
    collect_constraints(&parse_clean(src))
}

fn codes(diags: &[Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.as_str()).collect()
}

const TWO_DOMAINS: &str = "domain Fast { clock = posedge, frequency = 100.mhz }\n\
                           domain Slow { clock = posedge, frequency = 25_175.khz }\n";

fn two_clock_module(attrs: &str, body: &str) -> String {
    format!(
        "{TWO_DOMAINS}{attrs}\nmodule M {{\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n    \
         in a : u8 @Fast\n    out y : u8 @Slow\n    reg r : u8 = 0\n    on fast_clk {{ r <= a }}\n{body}\n}}\n"
    )
}

// ═══ Saatler ve periyot ═══════════════════════════════════════════

#[test]
fn domain_frequency_becomes_period_in_ps() {
    let r = collect(&two_clock_module("", "    y = sync(r, slow_clk)"));
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
    let m = &r.modules[0];
    assert_eq!(m.module, "M");
    assert_eq!(m.clocks.len(), 2);
    assert_eq!(m.clocks[0].port, "fast_clk");
    assert_eq!(m.clocks[0].freq_hz, Some(100_000_000));
    assert_eq!(m.clocks[0].period_ps(), Some(10_000));
    assert_eq!(m.clocks[1].freq_hz, Some(25_175_000));
    // 1e12 / 25175000 = 39721.947 → en yakına yuvarlanır.
    assert_eq!(m.clocks[1].period_ps(), Some(39_722));
}

#[test]
fn frequency_units_hz_khz_mhz_ghz_and_bare() {
    for (lit, hz) in [
        ("50.hz", 50u64),
        ("25_175.khz", 25_175_000),
        ("100.mhz", 100_000_000),
        ("1.ghz", 1_000_000_000),
        ("25175000", 25_175_000),
    ] {
        let src = format!(
            "domain D {{ clock = posedge, frequency = {lit} }}\nmodule M {{\n    in clk : clock @D\n    in a : u8\n    out y : u8\n    y = a\n}}\n"
        );
        let r = collect(&src);
        assert!(
            r.diagnostics.is_empty(),
            "{lit}: {:?}",
            codes(&r.diagnostics)
        );
        assert_eq!(r.modules[0].clocks[0].freq_hz, Some(hz), "{lit}");
    }
}

#[test]
fn unknown_frequency_unit_is_e0017() {
    let r = collect("domain D { clock = posedge, frequency = 100.thz }\nmodule M {\n    in clk : clock @D\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
    assert_eq!(r.modules[0].clocks[0].freq_hz, None);
}

#[test]
fn unannotated_clock_port_is_its_own_domain_without_frequency() {
    let r =
        collect("module M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    let c = &r.modules[0].clocks[0];
    assert_eq!(c.domain, "clk");
    assert_eq!(c.freq_hz, None);
    assert!(c.domain_span.is_none());
}

#[test]
fn module_without_clock_port_has_no_constraints_entry() {
    let r = collect("module M {\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert!(r.modules.is_empty());
    assert!(r.diagnostics.is_empty());
}

// ═══ @timing saat gereksinimleri ══════════════════════════════════

#[test]
fn timing_exact_supplies_frequency_when_domain_has_none() {
    let r = collect("@timing(clk = 100.mhz)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
    let c = &r.modules[0].clocks[0];
    assert_eq!(c.freq_hz, Some(100_000_000));
    assert_eq!(
        c.freq_source,
        Some(volt_hir::constraints::FreqSource::Timing)
    );
}

#[test]
fn timing_exact_matching_domain_is_silent() {
    let r = collect("domain D { clock = posedge, frequency = 100.mhz }\n@timing(clk = 100000000)\nmodule M {\n    in clk : clock @D\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
}

#[test]
fn timing_exact_conflicting_with_domain_is_e0017_with_domain_label() {
    let r = collect("domain D { clock = posedge, frequency = 100.mhz }\n@timing(clk = 50.mhz)\nmodule M {\n    in clk : clock @D\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
    let d = &r.diagnostics[0];
    assert!(
        d.message.contains("50000000") && d.message.contains("100000000"),
        "{}",
        d.message
    );
    assert_eq!(d.spans.len(), 2, "alan bildirimi ikincil etiket almalı");
    // Model alan değerini korur.
    assert_eq!(r.modules[0].clocks[0].freq_hz, Some(100_000_000));
}

#[test]
fn timing_at_least_met_is_silent_and_keeps_domain_value() {
    let r = collect("domain D { clock = posedge, frequency = 100.mhz }\n@timing(clk >= 40.mhz)\nmodule M {\n    in clk : clock @D\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
    assert_eq!(r.modules[0].clocks[0].freq_hz, Some(100_000_000));
}

#[test]
fn timing_at_least_not_met_is_e0017() {
    let r = collect("domain D { clock = posedge, frequency = 20.mhz }\n@timing(clk >= 40.mhz)\nmodule M {\n    in clk : clock @D\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
    assert!(
        r.diagnostics[0].message.contains(">= 40000000"),
        "{}",
        r.diagnostics[0].message
    );
}

#[test]
fn timing_on_unknown_clock_port_is_e0017() {
    let r = collect("@timing(nope >= 40.mhz)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
    assert!(r.diagnostics[0].spans[0].label.contains("not a clock port"));
}

#[test]
fn timing_upper_bound_on_clock_is_e0017() {
    let r = collect("@timing(clk <= 40.mhz)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
}

#[test]
fn empty_timing_is_e0017() {
    let r = collect(
        "@timing\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n",
    );
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
}

// ═══ Yol kısıtları ════════════════════════════════════════════════

#[test]
fn max_delay_and_min_delay_forms() {
    let r = collect("@timing(max_delay(a, y) <= 5.ns, min_delay(a, y) >= 250.ps)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    reg r : u8 = 0\n    on clk { r <= a }\n    y = r\n}\n");
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
    let p = &r.modules[0].paths;
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].kind, PathKind::MaxDelay(5_000));
    assert_eq!(p[0].from, Some(Target::Port("a".into())));
    assert_eq!(p[0].to, Some(Target::Port("y".into())));
    assert_eq!(p[1].kind, PathKind::MinDelay(250));
}

#[test]
fn delay_without_unit_is_e0017() {
    let r = collect("@timing(max_delay(a, y) <= 5)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
    assert!(
        r.diagnostics[0].spans[0].label.contains("unit"),
        "{:?}",
        r.diagnostics[0].spans[0].label
    );
}

#[test]
fn max_delay_with_wrong_operator_is_e0017() {
    let r = collect("@timing(max_delay(a, y) >= 5.ns)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
}

#[test]
fn false_path_from_register_to_port() {
    let r = collect("@false_path(from = cfg_r, to = y)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    reg cfg_r : u8 = 0\n    on clk { cfg_r <= a }\n    y = cfg_r\n}\n");
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
    let p = &r.modules[0].paths[0];
    assert_eq!(p.kind, PathKind::FalsePath);
    assert_eq!(p.from, Some(Target::Cells("cfg_r".into())));
    assert_eq!(p.to, Some(Target::Port("y".into())));
}

#[test]
fn false_path_on_let_is_e0017_not_an_endpoint() {
    let r = collect("@false_path(from = tmp, to = y)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    let tmp : u8 = a + 1\n    y = tmp\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
    assert!(r.diagnostics[0].spans[0].label.contains("wire/let"));
}

#[test]
fn false_path_on_unknown_name_is_e0017() {
    let r = collect("@false_path(from = ghost, to = y)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
}

#[test]
fn false_path_on_module_without_endpoints_is_e0017() {
    let r = collect("@false_path\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
}

#[test]
fn multicycle_on_register_statement_targets_that_register() {
    let r = collect("module M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    @multicycle(2)\n    reg r : u8 = 0\n    on clk { r <= a }\n    y = r\n}\n");
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
    let p = &r.modules[0].paths[0];
    assert_eq!(p.kind, PathKind::Multicycle(2));
    assert_eq!(p.from, None);
    assert_eq!(p.to, Some(Target::Cells("r".into())));
}

#[test]
fn multicycle_on_module_with_from_to_cycles() {
    let r = collect("@multicycle(from = a, to = r, cycles = 3)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    reg r : u8 = 0\n    on clk { r <= a }\n    y = r\n}\n");
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
    assert_eq!(r.modules[0].paths[0].kind, PathKind::Multicycle(3));
}

#[test]
fn multicycle_without_cycles_is_e0017() {
    let r = collect("@multicycle(from = a, to = y)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
}

#[test]
fn false_path_on_input_port_is_from_that_port() {
    let r = collect("module M {\n    in clk : clock\n    @false_path\n    in cfg : u8\n    out y : u8\n    reg r : u8 = 0\n    on clk { r <= cfg }\n    y = r\n}\n");
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
    let p = &r.modules[0].paths[0];
    assert_eq!(p.from, Some(Target::Port("cfg".into())));
    assert_eq!(p.to, None);
}

#[test]
fn timing_on_a_port_or_register_is_e0017() {
    let r = collect("module M {\n    @timing(clk = 100.mhz)\n    in clk : clock\n    in a : u8\n    out y : u8\n    @timing(clk = 100.mhz)\n    reg r : u8 = 0\n    on clk { r <= a }\n    y = r\n}\n");
    assert_eq!(codes(&r.diagnostics), vec!["E0017", "E0017"]);
}

// ═══ Gruplar ve köprüler ══════════════════════════════════════════

#[test]
fn two_domains_form_two_async_groups() {
    let r = collect(&two_clock_module("", "    y = sync(r, slow_clk)"));
    let m = &r.modules[0];
    assert_eq!(
        m.groups,
        vec![vec!["fast_clk".to_string()], vec!["slow_clk".to_string()]]
    );
    assert_eq!(m.async_groups().len(), 2);
}

#[test]
fn same_domain_clocks_share_a_group_and_no_clock_groups_emitted() {
    let r = collect("domain D { clock = posedge, frequency = 100.mhz }\nmodule M {\n    in clk_a : clock @D\n    in clk_b : clock @D\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    let m = &r.modules[0];
    assert_eq!(m.groups.len(), 1);
    assert!(m.async_groups().is_empty());
}

#[test]
fn clock_without_frequency_leaves_the_async_groups() {
    let r = collect("domain Fast { clock = posedge, frequency = 100.mhz }\ndomain Slow { clock = posedge }\nmodule M {\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n    in a : u8\n    out y : u8\n    y = a\n}\n");
    let m = &r.modules[0];
    assert_eq!(m.groups.len(), 2);
    assert!(
        m.async_groups().is_empty(),
        "tek tanımlı saatle grup üretilmez"
    );
}

#[test]
fn sync_bridge_from_register_into_stage0() {
    let r = collect(&two_clock_module("", "    y = sync(r, slow_clk)"));
    let b = &r.modules[0].bridges[0];
    assert_eq!(b.kind, "sync");
    assert_eq!(b.name, "sync_r");
    assert_eq!(b.from_clock.as_deref(), Some("fast_clk"));
    assert_eq!(b.to_clock.as_deref(), Some("slow_clk"));
    assert_eq!(b.rules.len(), 1);
    assert_eq!(b.rules[0].kind, PathKind::FalsePath);
    assert_eq!(b.rules[0].from, Some(Target::Cells("r".into())));
    assert_eq!(b.rules[0].to, Some(Target::Cells("sync_r_stage0".into())));
    assert_eq!(b.async_regs, vec!["sync_r_stage0", "sync_r_stage1"]);
}

#[test]
fn sync3_has_three_async_regs() {
    let r = collect(&two_clock_module("", "    y = sync3(r, slow_clk)"));
    let b = &r.modules[0].bridges[0];
    assert_eq!(b.kind, "sync3");
    assert_eq!(b.async_regs.len(), 3);
}

#[test]
fn sync_of_annotated_port_uses_capture_register() {
    // Port başka alandan geliyor: sv-emit `sync_<src>_src` yakalama
    // register'ı üretir; geçiş oradan stage0'a.
    let r = collect(&two_clock_module("", "    y = sync(a, slow_clk)"));
    let b = &r.modules[0].bridges[0];
    assert_eq!(b.rules[0].from, Some(Target::Cells("sync_a_src".into())));
    assert_eq!(b.from_clock.as_deref(), Some("fast_clk"));
}

#[test]
fn sync_of_let_traces_source_clock_through_expression() {
    let src = two_clock_module("", "    let n : u8 = r + 1\n    y = sync(n, slow_clk)");
    let r = collect(&src);
    let b = &r.modules[0].bridges[0];
    assert_eq!(b.from_clock.as_deref(), Some("fast_clk"));
    assert_eq!(b.rules[0].from, Some(Target::Clock("fast_clk".into())));
}

#[test]
fn builtin_bridges_async_fifo_handshake_pulse_ram() {
    let src = format!(
        "{TWO_DOMAINS}module M {{\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n    in d : u8 @Fast\n    in go : bool @Fast\n    out q : u8 @Slow\n    out v : bool @Slow\n    out p : bool @Slow\n    out m : bool @Slow\n    \
         let f = AsyncFifo<u8, 4> {{ wr_clk: fast_clk, wr_data: d, wr_en: go, rd_clk: slow_clk, rd_en: v }}\n    \
         let h = HandshakeSync<u8> {{ src_clk: fast_clk, data_in: d, send: go, dst_clk: slow_clk }}\n    \
         let ps = PulseSync {{ src_clk: fast_clk, pulse_in: go, dst_clk: slow_clk }}\n    \
         let ram = AsyncDualPortRam<bool, 16> {{ wr_clk: fast_clk, wr_addr: 0, wr_data: go, wr_en: go, rd_clk: slow_clk, rd_addr: 1 }}\n    \
         q = f.rd_data\n    v = h.valid\n    p = ps.pulse_out\n    m = ram.rd_data\n}}\n"
    );
    let r = collect(&src);
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
    let b = &r.modules[0].bridges;
    let kinds: Vec<&str> = b.iter().map(|b| b.kind).collect();
    assert_eq!(
        kinds,
        vec![
            "AsyncFifo",
            "HandshakeSync",
            "PulseSync",
            "AsyncDualPortRam"
        ]
    );
    assert_eq!(b[0].rules.len(), 3);
    assert_eq!(b[0].rules[0].from, Some(Target::Cells("f_rgray".into())));
    assert_eq!(b[0].rules[0].to, Some(Target::Cells("f_rgray_s0".into())));
    assert_eq!(
        b[0].async_regs,
        vec!["f_rgray_s0", "f_rgray_s1", "f_wgray_s0", "f_wgray_s1"]
    );
    assert_eq!(b[1].rules.len(), 3);
    assert_eq!(b[1].async_regs.len(), 4);
    assert_eq!(b[2].rules[0].to, Some(Target::Cells("ps_sync0".into())));
    assert_eq!(b[2].async_regs, vec!["ps_sync0", "ps_sync1", "ps_sync2"]);
    assert_eq!(b[3].rules[0].from, Some(Target::Cells("ram_mem".into())));
    assert_eq!(b[3].rules[0].to, Some(Target::Cells("ram_rd_data".into())));
    assert!(b[3].async_regs.is_empty());
    for bridge in b {
        assert_eq!(bridge.from_clock.as_deref(), Some("fast_clk"));
        assert_eq!(bridge.to_clock.as_deref(), Some("slow_clk"));
    }
}

#[test]
fn sub_module_bridges_get_instance_prefix_and_mapped_clocks() {
    let src = format!(
        "{TWO_DOMAINS}module Sub {{\n    in f : clock @Fast\n    in s : clock @Slow\n    in a : bool @Fast\n    out y : bool @Slow\n    y = sync(a, s)\n}}\n\
         @false_path(from = ctl, to = out_r)\nmodule Sub2 {{\n    in f : clock @Fast\n    in ctl : bool\n    out o : bool\n    reg out_r : bool = false\n    on f {{ out_r <= ctl }}\n    o = out_r\n}}\n\
         module Top {{\n    in fast_clk : clock @Fast\n    in slow_clk : clock @Slow\n    in x : bool @Fast\n    out y : bool @Slow\n    out o : bool @Fast\n    \
         let u = Sub {{ f: fast_clk, s: slow_clk, a: x }}\n    let w = Sub2 {{ f: fast_clk, ctl: x }}\n    y = u.y\n    o = w.o\n}}\n"
    );
    let r = collect(&src);
    assert!(r.diagnostics.is_empty(), "{:?}", codes(&r.diagnostics));
    let top = r.modules.iter().find(|m| m.module == "Top").unwrap();
    assert_eq!(top.bridges.len(), 1);
    let b = &top.bridges[0];
    assert_eq!(b.name, "u/sync_a");
    assert_eq!(b.from_clock.as_deref(), Some("fast_clk"));
    assert_eq!(b.to_clock.as_deref(), Some("slow_clk"));
    assert_eq!(b.rules[0].from, Some(Target::Cells("u/sync_a_src".into())));
    assert_eq!(b.async_regs[0], "u/sync_a_stage0");
    // Alt modülün @false_path'i üstte önekli, alt modülün portu get_pins.
    assert_eq!(top.paths.len(), 1);
    assert_eq!(top.paths[0].from, Some(Target::Pin("w/ctl".into())));
    assert_eq!(top.paths[0].to, Some(Target::Cells("w/out_r".into())));
    // Alt modülün kendi girişi öneksiz.
    let sub2 = r.modules.iter().find(|m| m.module == "Sub2").unwrap();
    assert_eq!(sub2.paths[0].from, Some(Target::Port("ctl".into())));
}

#[test]
fn sub_module_clock_requirement_checked_against_parent_frequency() {
    let src = "domain Slow { clock = posedge, frequency = 10.mhz }\n\
               @timing(clk >= 40.mhz)\nmodule Fastish {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n\
               module Top {\n    in slow_clk : clock @Slow\n    in a : u8\n    out y : u8\n    let u = Fastish { clk: slow_clk, a: a }\n    y = u.y\n}\n";
    let r = collect(src);
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
    // Alt modül kendi başına (frekansı bilinmeyen saat) gereksinimi doldurur.
    let sub = r.modules.iter().find(|m| m.module == "Fastish").unwrap();
    assert_eq!(sub.clocks[0].freq_hz, Some(40_000_000));
}

#[test]
fn e0017_reported_once_per_location_across_parents() {
    let src = "@timing(max_delay(a, y) <= 5)\nmodule Sub {\n    in clk : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n\
               module A {\n    in clk : clock\n    in a : u8\n    out y : u8\n    let u = Sub { clk, a }\n    y = u.y\n}\n\
               module B {\n    in clk : clock\n    in a : u8\n    out y : u8\n    let u = Sub { clk, a }\n    y = u.y\n}\n";
    let r = collect(src);
    assert_eq!(codes(&r.diagnostics), vec!["E0017"]);
}

// ═══ W0022 yalnız istek üzerine ═══════════════════════════════════

#[test]
fn missing_frequency_warnings_once_per_domain() {
    let src = "domain Sys { clock = posedge }\n\
               module A {\n    in clk : clock @Sys\n    in a : u8\n    out y : u8\n    y = a\n}\n\
               module B {\n    in clk : clock @Sys\n    in free : clock\n    in a : u8\n    out y : u8\n    y = a\n}\n";
    let r = collect(src);
    assert!(r.diagnostics.is_empty(), "W0022 collect'te üretilmez");
    let w = r.missing_frequency_warnings();
    assert_eq!(
        codes(&w),
        vec!["W0022", "W0022"],
        "Sys bir kez + anotasyonsuz 'free'"
    );
    assert!(w[0].message.contains("'Sys'"), "{}", w[0].message);
    assert!(w[1].message.contains("'free'"), "{}", w[1].message);
}

#[test]
fn analyze_reports_e0017_but_never_w0022() {
    let src = "domain Sys { clock = posedge }\n@timing(max_delay(a, y) <= 5)\nmodule M {\n    in clk : clock @Sys\n    in a : u8\n    out y : u8\n    reg r : u8 = 0\n    on clk { r <= a }\n    y = r\n}\n";
    let a = analyze(&parse_clean(src));
    let codes = a.error_codes();
    assert!(codes.contains(&"E0017"), "{codes:?}");
    assert!(
        !codes.contains(&"W0022"),
        "W0022 yalnız --emit=sdc'de: {codes:?}"
    );
}

#[test]
fn enforced_timing_attributes_no_longer_warn_w0021() {
    let src = "@timing(clk = 100.mhz)\n@false_path(from = a, to = y)\n@multicycle(from = a, to = y, cycles = 2)\nmodule M {\n    in clk : clock\n    in a : u8\n    out y : u8\n    reg r : u8 = 0\n    on clk { r <= a }\n    y = r\n}\n";
    let a = analyze(&parse_clean(src));
    assert!(a.diagnostics.is_empty(), "{:?}", a.error_codes());
}

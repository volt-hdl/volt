//! Simülasyon kontrat izleyicileri — ADR-0064 (`SvaMode::Simulation`).
//!
//! İzleyici, `volt verify`'ın immediate kalıbının DPI geri çağrılı
//! karşılığıdır: aynı kenar, aynı reset koruması, aynı `prev()` zinciri,
//! aynı property adları; formal varsayımlar ve init blokları yoktur.

use volt_span::FileId;
use volt_sv_emit::{emit_full, uses_sim_contracts, EmitOutput, SvaMode};

fn emit(src: &str, mode: SvaMode) -> EmitOutput {
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
            .any(|d| d.code.as_str().starts_with('E')),
        "emit hatasız olmalı: {:?}",
        out.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    out
}

fn sim(src: &str) -> String {
    emit(src, SvaMode::Simulation).sv
}

const COUNTER: &str = "\
module Cnt {
    in  clk   : clock
    in  en    : bool
    out count : u8

    invariant: count < 5
    ensures: !en || count != 9
    requires: en || !prev(en)
    assume: count != 200
    cover: count == 3

    reg c : u8 = 0
    on clk {
        if en { c <= c + 1 }
    }
    count = c
}
";

#[test]
fn invariant_becomes_reset_guarded_dpi_monitor() {
    let sv = sim(COUNTER);
    assert!(
        sv.contains(
            "    always @(posedge clk)\n        if (!(rst)) if (!(count < 8'd5)) volt_contract_fail(\"Cnt.inv_0\");"
        ),
        "{sv}"
    );
    assert!(sv.contains("// invariant from test.volt:6"), "{sv}");
}

#[test]
fn assert_and_assume_kinds_share_the_violation_callback_with_their_own_ids() {
    let sv = sim(COUNTER);
    for id in ["Cnt.ens_0", "Cnt.req_0", "Cnt.asm_0"] {
        assert!(
            sv.contains(&format!("volt_contract_fail(\"{id}\");")),
            "{id}: {sv}"
        );
    }
}

#[test]
fn cover_counts_in_sv_and_reports_once_in_final() {
    let sv = sim(COUNTER);
    assert!(sv.contains("    longint volt_hits_cov_0 = 0;"), "{sv}");
    assert!(
        sv.contains("if (!(rst)) if (count == 8'd3) volt_hits_cov_0 <= volt_hits_cov_0 + 1;"),
        "{sv}"
    );
    assert!(
        sv.contains("    final volt_cover_report(\"Cnt.cov_0\", volt_hits_cov_0);"),
        "{sv}"
    );
}

#[test]
fn only_the_used_dpi_imports_are_declared_first_in_the_body() {
    let sv = sim(COUNTER);
    let fail = "    import \"DPI-C\" context function void volt_contract_fail(input string id);";
    let cover = "    import \"DPI-C\" context function void volt_cover_report(input string id, input longint hits);";
    assert!(sv.contains(fail) && sv.contains(cover), "{sv}");
    // Bildirimler gövdenin başında, kullanımdan önce.
    assert!(sv.find(fail) < sv.find("logic [7:0] c;"), "{sv}");

    let only_cover = sim("module C {\n    in clk : clock\n    in x : bool\n    cover: x\n}\n");
    assert!(only_cover.contains("volt_cover_report"), "{only_cover}");
    assert!(!only_cover.contains("volt_contract_fail"), "{only_cover}");
}

#[test]
fn prev_uses_helper_registers_that_reset_to_zero_like_formal() {
    let sv = sim(COUNTER);
    assert!(!sv.contains("$past"), "{sv}");
    assert!(sv.contains("logic past_en_1;"), "{sv}");
    assert!(sv.contains("past_en_1 <= '0;"), "{sv}");
    assert!(
        sv.contains("if (!(en || !past_en_1)) volt_contract_fail(\"Cnt.req_0\");"),
        "{sv}"
    );
}

#[test]
fn no_formal_constructs_in_simulation_mode() {
    let sv = sim(COUNTER);
    for formal in [
        "initial assume",
        "assert (",
        "assume (",
        "cover (",
        "property",
    ] {
        assert!(!sv.contains(formal), "`{formal}` üretilmemeli: {sv}");
    }
}

#[test]
fn property_names_match_the_verify_flow() {
    let names = |mode| -> Vec<(String, String, &'static str)> {
        emit(COUNTER, mode)
            .sva_props
            .into_iter()
            .map(|p| (p.module_name, p.name, p.keyword))
            .collect()
    };
    assert_eq!(names(SvaMode::Simulation), names(SvaMode::Immediate));
    assert_eq!(names(SvaMode::Simulation).len(), 5);
}

#[test]
fn rtl_part_is_unchanged_without_contracts() {
    let src = "module P {\n    in clk : clock\n    in d : u4\n    out q : u4\n    reg r : u4 = 0\n    on clk { r <= d }\n    q = r\n}\n";
    let simulated = sim(src);
    assert_eq!(simulated, emit(src, SvaMode::None).sv);
    assert!(!uses_sim_contracts(&simulated));
}

#[test]
fn uses_sim_contracts_detects_monitors() {
    assert!(uses_sim_contracts(&sim(COUNTER)));
    assert!(!uses_sim_contracts(&emit(COUNTER, SvaMode::None).sv));
    assert!(!uses_sim_contracts(&emit(COUNTER, SvaMode::Immediate).sv));
}

#[test]
fn clockless_module_gets_no_monitor() {
    let sv =
        sim("module Comb {\n    in a : u4\n    out b : u4\n    invariant: b == a\n    b = a\n}\n");
    assert!(!sv.contains("volt_contract_fail"), "{sv}");
    assert!(!uses_sim_contracts(&sv));
}

#[test]
fn negedge_active_low_domain_uses_its_edge_and_reset_condition() {
    let sv = sim("\
domain D {
    clock = negedge
    reset = async active_low
}
module N {
    in clk : clock @D
    in x   : bool  @D
    invariant: x
}
");
    assert!(sv.contains("always @(negedge clk)"), "{sv}");
    assert!(
        sv.contains("if (!(!rst_n)) if (!(x)) volt_contract_fail(\"N.inv_0\");"),
        "{sv}"
    );
}

const FIFO: &str = include_str!("../../../tests/ui/pass/30_sync_fifo.volt");

#[test]
fn stdlib_primitive_contracts_become_monitors_without_formal_setup() {
    let sv = sim(FIFO);
    assert!(
        sv.contains("// simulation contracts: 'fifo' (SyncFifo)"),
        "{sv}"
    );
    assert!(
        sv.contains("volt_contract_fail(\"FifoBuffer.fifo_inv_0\");"),
        "{sv}"
    );
    assert!(
        sv.contains("final volt_cover_report(\"FifoBuffer.fifo_cov_0\", volt_hits_fifo_cov_0);"),
        "{sv}"
    );
    // Formal kurulum (reset varsayımı, init bloğu) simülasyonda yok.
    assert!(!sv.contains("initial assume"), "{sv}");
    assert!(!sv.contains("formal init"), "{sv}");
    // Yosys için yazılmış metin: genişlik uyarısı yalnız izleyicide susar.
    assert!(
        sv.contains("    // verilator lint_off WIDTH\n    always"),
        "{sv}"
    );
    assert_eq!(
        sv.matches("lint_off WIDTH").count(),
        sv.matches("lint_on WIDTH").count()
    );
}

#[test]
fn stdlib_primitive_props_carry_the_primitive_name() {
    let props = emit(FIFO, SvaMode::Simulation).sva_props;
    assert!(!props.is_empty());
    assert!(props.iter().all(|p| p.primitive == Some("SyncFifo")));
    let user = emit(COUNTER, SvaMode::Simulation).sva_props;
    assert!(user.iter().all(|p| p.primitive.is_none()));
}

#[test]
fn immediate_primitive_output_keeps_its_formal_setup() {
    // Regresyon: Simulation dalı Immediate çıktısına sızmamalı.
    let sv = emit(FIFO, SvaMode::Immediate).sv;
    assert!(
        sv.contains("// formal contracts: 'fifo' (SyncFifo)"),
        "{sv}"
    );
    assert!(sv.contains("formal init"), "{sv}");
    assert!(sv.contains("// volt:fifo_inv_0"), "{sv}");
    assert!(!sv.contains("volt_contract_fail"), "{sv}");
    assert!(!sv.contains("lint_off"), "{sv}");
}

#[test]
fn handshake_protocol_contracts_are_monitored() {
    let sv = sim(include_str!(
        "../../../tests/ui/pass/67_handshake_contracts.volt"
    ));
    assert!(sv.contains("volt_contract_fail("), "{sv}");
}

#[test]
fn submodule_contracts_live_in_their_own_module() {
    let src = "\
module Sub {
    in  clk : clock
    in  x   : u8
    invariant: x < 5
}
module Top {
    in  clk : clock
    in  d   : u8
    let u = Sub { clk: clk, x: d }
}
";
    let sv = sim(src);
    let sub = &sv[sv.find("module Sub").expect("Sub")..sv.find("module Top").expect("Top")];
    assert!(sub.contains("volt_contract_fail(\"Sub.inv_0\");"), "{sub}");
    let top = &sv[sv.find("module Top").expect("Top")..];
    assert!(!top.contains("volt_contract_fail"), "{top}");
}

#[test]
fn mmio_auto_contracts_are_monitored() {
    let sv = sim(include_str!("../../../tests/ui/pass/57_mmio_basic.volt"));
    assert!(uses_sim_contracts(&sv), "{sv}");
}

// ═══ Testbench (C++) ══════════════════════════════════════════════

mod testbench {
    use volt_sv_emit::{
        run_testbench_cpp, run_testbench_cpp_with, test_testbench_cpp, test_testbench_cpp_with,
        SimPort, TbStep, TbTest, TbValue,
    };

    fn ports() -> Vec<SimPort> {
        let port = |name: &str, is_input, is_clock| SimPort {
            name: name.to_string(),
            is_input,
            is_clock,
            reset: None,
        };
        vec![
            port("clk", true, true),
            port("en", true, false),
            port("count", false, false),
        ]
    }

    fn tests(steps: Vec<TbStep>) -> Vec<TbTest> {
        vec![TbTest {
            name: "t".to_string(),
            steps,
        }]
    }

    #[test]
    fn contracts_off_is_byte_identical_to_the_legacy_generator() {
        let t = tests(vec![TbStep::Step(3), TbStep::Reset]);
        assert_eq!(
            test_testbench_cpp_with("Cnt", &ports(), &t, false),
            test_testbench_cpp("Cnt", &ports(), &t)
        );
        assert_eq!(
            run_testbench_cpp_with("Cnt", &ports(), 5, None, false),
            run_testbench_cpp("Cnt", &ports(), 5, None)
        );
    }

    #[test]
    fn contract_testbench_defines_the_dpi_callbacks_and_counts_cycles() {
        let cpp = test_testbench_cpp_with("Cnt", &ports(), &tests(vec![TbStep::Step(3)]), true);
        assert!(cpp.contains("#include \"svdpi.h\""), "{cpp}");
        assert!(
            cpp.contains("extern \"C\" void volt_contract_fail(const char* id)"),
            "{cpp}"
        );
        assert!(
            cpp.contains("extern \"C\" void volt_cover_report(const char* id, long long hits)"),
            "{cpp}"
        );
        assert!(cpp.contains("svGetNameFromScope(svGetScope())"), "{cpp}");
        // Çevrim sayacı posedge'den önce artar.
        let cycle = cpp.split("run_cycle(TOP* dut").nth(1).expect("run_cycle");
        assert!(
            cycle.find("++volt_cycle;") < cycle.find("dut->clk = 1;"),
            "{cpp}"
        );
    }

    #[test]
    fn each_test_restarts_the_cycle_counter_after_its_reset() {
        let cpp = test_testbench_cpp_with("Cnt", &ports(), &tests(vec![TbStep::Step(1)]), true);
        assert!(
            cpp.contains("    apply_reset(&dut, ctx);\n    volt_contracts_begin();\n"),
            "{cpp}"
        );
        assert!(
            cpp.contains("    volt_cover_summary();\n    return failed == 0 ? 0 : 1;"),
            "{cpp}"
        );
    }

    #[test]
    fn step_stops_at_the_violating_cycle_and_fails_the_test() {
        let cpp = test_testbench_cpp_with("Cnt", &ports(), &tests(vec![TbStep::Step(200)]), true);
        assert!(
            cpp.contains(
                "for (unsigned long long s = 0; s < 200ULL; ++s) { run_cycle(&dut, ctx); if (!volt_violations.empty()) break; }"
            ),
            "{cpp}"
        );
        assert!(
            cpp.contains(
                "std::printf(\"VOLT-CONTRACT-FAIL %s cycle=%llu inst=%s\\n\", volt_v.id.c_str(), volt_v.cycle, volt_v.inst.c_str());"
            ),
            "{cpp}"
        );
        // Son eşleşme test gövdesindedir (ilki prelude'daki `volt run` dökümü).
        let fail = cpp
            .rsplit("VOLT-CONTRACT-FAIL %s")
            .next()
            .expect("ihlal bloğu");
        let final_at = fail.find("dut.final();").expect("final");
        assert!(final_at < fail.find("return false;").expect("return"));
    }

    #[test]
    fn reset_step_is_also_followed_by_a_violation_check() {
        let cpp = test_testbench_cpp_with("Cnt", &ports(), &tests(vec![TbStep::Reset]), true);
        let after = cpp
            .split("volt_contracts_begin();")
            .nth(1)
            .expect("test gövdesi");
        assert!(after.contains("apply_reset(&dut, ctx);\n    if (!volt_violations.empty()) {"));
    }

    #[test]
    fn violation_inside_a_loop_reports_the_loop_counters() {
        let cpp = test_testbench_cpp_with(
            "Cnt",
            &ports(),
            &tests(vec![TbStep::For {
                var: "i".to_string(),
                start: TbValue::Lit(0),
                end: TbValue::Lit(3),
                body: vec![TbStep::StepBy(TbValue::Var("i".to_string()))],
            }]),
            true,
        );
        assert!(
            cpp.contains(
                "VOLT-CONTRACT-FAIL %s cycle=%llu loop=i=%llu inst=%s\\n\", volt_v.id.c_str(), volt_v.cycle, v_i, volt_v.inst.c_str());"
            ),
            "{cpp}"
        );
        assert!(
            cpp.contains(
                "s < volt_n; ++s) { run_cycle(&dut, ctx); if (!volt_violations.empty()) break; } }"
            ),
            "{cpp}"
        );
    }

    #[test]
    fn run_testbench_reports_all_violations_and_covers_at_the_end() {
        let cpp = run_testbench_cpp_with("Cnt", &ports(), 50, None, true);
        assert!(cpp.contains("    apply_reset(&dut, &ctx);\n    volt_contracts_begin();\n"));
        assert!(cpp.contains(
            "    dut.final();\n    volt_contracts_dump();\n    volt_cover_summary();\n    return 0;"
        ));
        // Koşu ihlalde durmaz: döngü gövdesinde denetim yok.
        assert!(!cpp.contains("break;"), "{cpp}");
    }
}

#[test]
fn unsupported_contract_is_skipped_with_w5001_keeping_verify_names() {
    let src = "module M {\n    in clk : clock\n    in a : u4\n    invariant: match a { 0 => true, _ => a != 7 }\n    invariant: a != 9\n}\n";
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    let out = emit_full(&parsed.ast, "test.volt", src, SvaMode::Simulation);
    let codes: Vec<&str> = out.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["W5001"]);
    assert!(!out.sv.contains("\"M.inv_0\""), "{}", out.sv);
    assert!(
        out.sv.contains("volt_contract_fail(\"M.inv_1\");"),
        "{}",
        out.sv
    );
    // Formal akış aynı ifadeyi hâlâ E0003 ile reddeder (davranış değişmedi).
    let formal = emit_full(&parsed.ast, "test.volt", src, SvaMode::Immediate);
    assert!(formal
        .diagnostics
        .iter()
        .any(|d| d.code.as_str() == "E0003"));
}

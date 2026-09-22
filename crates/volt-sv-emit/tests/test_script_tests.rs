//! Test betiği → C++ üretimi (ADR-0058): değişken, dizi, çalışma
//! zamanı `for`, `load` ve çalışma zamanı hata raporu.

use volt_ast::TestBinOp;
use volt_sv_emit::{
    load_config_vlt, test_testbench_cpp, SimPort, TbAssertKind, TbPortCheck, TbStep, TbTest,
    TbValue,
};

fn ports() -> Vec<SimPort> {
    let port = |name: &str, is_input, is_clock| SimPort {
        name: name.into(),
        is_input,
        is_clock,
        reset: None,
    };
    vec![
        port("clk", true, true),
        port("a", true, false),
        port("sum", false, false),
    ]
}

fn cpp_of(steps: Vec<TbStep>) -> String {
    test_testbench_cpp(
        "Adder",
        &ports(),
        &[TbTest {
            name: "t".into(),
            steps,
        }],
    )
}

fn var(name: &str) -> TbValue {
    TbValue::Var(name.into())
}

fn binary(op: TestBinOp, lhs: TbValue, rhs: TbValue) -> TbValue {
    TbValue::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

fn assert_eq_step(left: TbValue, right: TbValue) -> TbStep {
    TbStep::Assert {
        kind: TbAssertKind::Eq,
        left,
        right,
        loc: "adder_test.volt:9".into(),
    }
}

#[test]
fn legacy_script_gets_no_runtime_prelude() {
    let cpp = cpp_of(vec![
        TbStep::SetPort {
            port: "a".into(),
            value: TbValue::Lit(1),
        },
        TbStep::Step(1),
        assert_eq_step(TbValue::Port("sum".into()), TbValue::Lit(1)),
    ]);
    assert!(!cpp.contains("volt_fault"), "ADR-0033 çıktısı değişmemeli");
    assert!(!cpp.contains("<cstddef>"));
    assert!(!cpp.contains("___024root"));
}

#[test]
fn for_is_a_cpp_loop_not_an_unrolling() {
    let cpp = cpp_of(vec![TbStep::For {
        var: "i".into(),
        start: TbValue::Lit(0),
        end: TbValue::Lit(100_000),
        body: vec![
            TbStep::SetPort {
                port: "a".into(),
                value: var("i"),
            },
            TbStep::Step(1),
        ],
    }]);
    assert!(cpp.contains("volt_lo_i = 0ULL, volt_hi_i = 100000ULL;"));
    assert!(cpp.contains("for (unsigned long long v_i = volt_lo_i; v_i < volt_hi_i; ++v_i) {"));
    assert_eq!(
        cpp.matches("dut.a = v_i;").count(),
        1,
        "gövde bir kez üretilir"
    );
    assert!(cpp.len() < 8_000, "100000 yineleme kodu büyütmemeli");
}

#[test]
fn loop_bounds_are_evaluated_once() {
    // Sınır ifadesi döngü koşulunda yeniden hesaplanmaz: gövde portu
    // değiştirse de yineleme sayısı sabittir.
    let cpp = cpp_of(vec![TbStep::For {
        var: "i".into(),
        start: TbValue::Lit(0),
        end: TbValue::Port("sum".into()),
        body: vec![TbStep::Step(1)],
    }]);
    assert!(cpp.contains("volt_hi_i = (unsigned long long)dut.sum;"));
    assert!(cpp.contains("v_i < volt_hi_i;"));
}

#[test]
fn array_is_embedded_as_static_data_with_length() {
    let cpp = cpp_of(vec![TbStep::LetArray {
        name: "rom".into(),
        data: (0..10).collect(),
    }]);
    assert!(cpp.contains("static const unsigned long long v_rom[] = {"));
    assert!(cpp.contains("0x0ULL, 0x1ULL, 0x2ULL, 0x3ULL, 0x4ULL, 0x5ULL, 0x6ULL, 0x7ULL,"));
    assert!(cpp.contains("0x8ULL, 0x9ULL,"));
    assert!(cpp.contains("static const std::size_t n_rom = 10;"));
    assert!(cpp.contains("#include <cstddef>"));
}

#[test]
fn index_goes_through_the_bounds_checked_helper() {
    let cpp = cpp_of(vec![
        TbStep::LetArray {
            name: "e".into(),
            data: vec![1, 2],
        },
        TbStep::Loc("adder_test.volt:9".into()),
        assert_eq_step(
            TbValue::Port("sum".into()),
            TbValue::Index {
                array: "e".into(),
                index: Box::new(var("i")),
            },
        ),
    ]);
    assert!(cpp.contains("volt_at(v_e, n_e, v_i)"));
    // Hata, değerler yargılanmadan ÖNCE raporlanır.
    let fault = cpp.find("if (volt_fault) {").expect("hata denetimi");
    let judge = cpp.find("if (!(volt_al == volt_ar)) {").expect("assert");
    assert!(fault < judge);
    assert!(cpp.contains("VOLT-ASSERT-FAIL %s adder_test.volt:9 left=%llu right=%llu"));
    assert!(cpp.contains("case 1: return \"index_out_of_bounds\";"));
}

#[test]
fn division_and_shifts_avoid_undefined_behaviour() {
    let cpp = cpp_of(vec![
        TbStep::Loc("adder_test.volt:4".into()),
        TbStep::LetScalar {
            name: "q".into(),
            value: binary(
                TestBinOp::Div,
                binary(TestBinOp::Shl, var("a"), var("b")),
                binary(TestBinOp::Rem, var("c"), TbValue::Lit(0)),
            ),
        },
    ]);
    assert!(cpp.contains(
        "const unsigned long long v_q = volt_div(volt_shl(v_a, v_b), volt_rem(v_c, 0ULL));"
    ));
    assert!(cpp.contains("return b ? a / b : volt_set_fault(2, a, b);"));
    assert!(cpp.contains("return b < 64 ? a << b : 0;"));
    assert!(cpp.contains("VOLT-ASSERT-FAIL %s adder_test.volt:4"));
}

#[test]
fn operators_are_fully_parenthesised() {
    let cpp = cpp_of(vec![TbStep::LetScalar {
        name: "v".into(),
        value: binary(
            TestBinOp::LogAnd,
            binary(
                TestBinOp::Eq,
                binary(TestBinOp::And, var("a"), TbValue::Lit(1)),
                TbValue::Lit(0),
            ),
            TbValue::Not(Box::new(var("c"))),
        ),
    }]);
    assert!(cpp.contains(
        "(unsigned long long)((((unsigned long long)((((v_a) & (1ULL))) == (0ULL))) != 0) && (((unsigned long long)((v_c) == 0)) != 0))"
    ));
}

#[test]
fn failure_inside_loops_reports_the_counters() {
    let cpp = cpp_of(vec![TbStep::For {
        var: "i".into(),
        start: TbValue::Lit(0),
        end: TbValue::Lit(4),
        body: vec![TbStep::For {
            var: "j".into(),
            start: TbValue::Lit(0),
            end: TbValue::Lit(4),
            body: vec![assert_eq_step(TbValue::Port("sum".into()), var("j"))],
        }],
    }]);
    assert!(cpp.contains(
        "VOLT-ASSERT-FAIL assert_eq adder_test.volt:9 left=%llu right=%llu loop=i=%llu,j=%llu\\n\", (unsigned long long)dut.sum, v_j, v_i, v_j);"
    ));
}

#[test]
fn step_by_expression_runs_that_many_cycles() {
    let cpp = cpp_of(vec![TbStep::StepBy(TbValue::Len("rom".into()))]);
    assert!(cpp.contains("const unsigned long long volt_n = (unsigned long long)n_rom;"));
    assert!(cpp.contains("s < volt_n; ++s) run_cycle(&dut, ctx);"));
}

#[test]
fn load_writes_through_rootp_and_survives_reset() {
    let cpp = cpp_of(vec![
        TbStep::LetArray {
            name: "rom".into(),
            data: vec![0x13],
        },
        TbStep::Loc("soc_test.volt:5".into()),
        TbStep::Load {
            target: "Adder__DOT__cpu__DOT__imem".into(),
            source: "rom".into(),
            elem_bits: Some(12),
        },
        TbStep::Reset,
    ]);
    assert!(cpp.contains("#include \"VAdder___024root.h\""));
    assert!(cpp.contains("static void volt_load(VlUnpacked<T, N>& dst"));
    assert!(cpp.contains("volt_load(dut.rootp->Adder__DOT__cpu__DOT__imem, v_rom, n_rom, 12U);"));
    assert!(cpp.contains("std::vector<std::function<void()>> volt_loads;"));
    // Yüklemeden sonra kombinasyonel mantık tazelenir.
    let load = cpp.find("volt_loads.push_back(volt_l);").expect("load");
    let eval = cpp[load..].find("dut.eval();").expect("eval");
    assert!(eval < 80);
    // reset() belleği siler; yüklemeler yeniden uygulanır.
    let reset = cpp.rfind("apply_reset(&dut, ctx);").expect("reset");
    assert!(cpp[reset..].contains("for (auto& volt_l : volt_loads) volt_l();"));
    assert!(cpp.contains("VOLT-ASSERT-FAIL %s soc_test.volt:5"));
}

#[test]
fn tests_without_load_keep_their_reset_untouched() {
    let with_load = TbTest {
        name: "a".into(),
        steps: vec![
            TbStep::LetArray {
                name: "rom".into(),
                data: vec![1],
            },
            TbStep::Load {
                target: "Adder__DOT__mem".into(),
                source: "rom".into(),
                elem_bits: None,
            },
        ],
    };
    let plain = TbTest {
        name: "b".into(),
        steps: vec![TbStep::Reset],
    };
    let cpp = test_testbench_cpp("Adder", &ports(), &[with_load, plain]);
    let second = &cpp[cpp.find("static bool test_1()").expect("ikinci test")..];
    let second = &second[..second.find("int main").expect("main")];
    assert!(
        !second.contains("volt_loads"),
        "load'suz test vektör kurmaz"
    );
    assert!(
        second.contains("volt_fault = 0;"),
        "hata durumu test başında sıfırlanır"
    );
}

#[test]
fn vlt_config_opens_only_the_named_memories() {
    let vlt = load_config_vlt(&[
        ("Soc".to_string(), "ram".to_string()),
        ("Core".to_string(), "imem".to_string()),
    ]);
    assert!(vlt.contains("`verilator_config\n"));
    assert!(vlt.contains("public_flat_rw -module \"Soc\" -var \"ram\"\n"));
    assert!(vlt.contains("public_flat_rw -module \"Core\" -var \"imem\"\n"));
    assert_eq!(vlt.matches("public_flat_rw").count(), 2);
}

#[test]
fn names_cannot_inject_printf_directives() {
    let cpp = test_testbench_cpp(
        "Adder",
        &ports(),
        &[TbTest {
            name: "100%s_done".into(),
            steps: vec![
                TbStep::Loc("50%_test.volt:3".into()),
                TbStep::LetScalar {
                    name: "q".into(),
                    value: binary(TestBinOp::Div, var("a"), var("b")),
                },
            ],
        }],
    );
    assert!(cpp.contains("VOLT-TEST-BEGIN 100%%s_done"));
    assert!(cpp.contains("VOLT-ASSERT-FAIL %s 50%%_test.volt:3"));
    assert!(!cpp.contains("100%s_done"));
}

#[test]
fn load_without_known_width_falls_back_to_the_cpp_type() {
    let cpp = cpp_of(vec![
        TbStep::LetArray {
            name: "d".into(),
            data: vec![1],
        },
        TbStep::Load {
            target: "Adder__DOT__mem".into(),
            source: "d".into(),
            elem_bits: None,
        },
    ]);
    assert!(cpp.contains("volt_load(dut.rootp->Adder__DOT__mem, v_d, n_d, 0U);"));
    assert!(cpp.contains("(bits >= 64 || (src[i] >> bits) == 0)"));
}

// ═══ Port genişlik denetimi (ADR-0059) ═══

fn checked(value: TbValue, bits: Option<u32>, signed: bool, type_name: &str) -> TbStep {
    TbStep::SetPortChecked {
        port: "a".into(),
        value,
        check: TbPortCheck {
            bits,
            signed,
            type_name: type_name.into(),
        },
    }
}

#[test]
fn proven_port_write_stays_unchecked() {
    let cpp = cpp_of(vec![TbStep::SetPort {
        port: "a".into(),
        value: TbValue::Lit(7),
    }]);
    assert!(cpp.contains("    dut.a = 7ULL;\n"));
    assert!(!cpp.contains("volt_port_fits"), "koruma yalnız gerekince");
}

#[test]
fn checked_port_write_guards_before_the_assignment() {
    let cpp = cpp_of(vec![
        TbStep::Loc("adder_test.volt:4".into()),
        checked(var("n"), Some(3), false, "u3"),
    ]);
    assert!(cpp.contains("static bool volt_port_fits("));
    let guard = cpp
        .find("if (!volt_port_fits(volt_pv, 3U, false)) {")
        .expect("koruma");
    let write = cpp
        .find("dut.a = volt_port_bits(volt_pv, 3U); }")
        .expect("yazma");
    assert!(guard < write, "sığmayan değer porta ulaşmamalı");
    assert!(cpp.contains("const unsigned long long volt_pv = v_n;"));
}

#[test]
fn port_overflow_report_names_port_type_and_width() {
    let cpp = cpp_of(vec![
        TbStep::Loc("adder_test.volt:4".into()),
        checked(var("n"), Some(3), false, "u3"),
    ]);
    assert!(cpp.contains(
        "VOLT-ASSERT-FAIL port_overflow adder_test.volt:4 left=%llu right=%llu port=a:u3\\n\", volt_pv, 3ULL);"
    ));
}

#[test]
fn port_overflow_inside_a_loop_reports_the_iteration() {
    let cpp = cpp_of(vec![TbStep::For {
        var: "i".into(),
        start: TbValue::Lit(0),
        end: TbValue::Lit(16),
        body: vec![
            TbStep::Loc("adder_test.volt:5".into()),
            checked(var("i"), Some(3), false, "u3"),
        ],
    }]);
    assert!(cpp.contains("left=%llu right=%llu loop=i=%llu port=a:u3\\n\", volt_pv, 3ULL, v_i);"));
}

#[test]
fn signed_port_check_allows_negative_numbers() {
    let cpp = cpp_of(vec![checked(var("n"), Some(8), true, "i8")]);
    assert!(cpp.contains("volt_port_fits(volt_pv, 8U, true)"));
    assert!(cpp.contains("dut.a = volt_port_bits(volt_pv, 8U); }"));
}

#[test]
fn unknown_width_falls_back_to_the_storage_type() {
    let cpp = cpp_of(vec![checked(var("n"), None, false, "?")]);
    assert!(cpp.contains("if (!volt_port_fits_type(dut.a, volt_pv)) {"));
    assert!(cpp.contains("dut.a = volt_pv; }"));
    assert!(cpp.contains("right=%llu port=a:?\\n\", volt_pv, 0ULL);"));
}

#[test]
fn faulting_value_is_reported_before_the_port_check() {
    let cpp = cpp_of(vec![
        TbStep::LetArray {
            name: "xs".into(),
            data: vec![1, 2],
        },
        checked(
            TbValue::Index {
                array: "xs".into(),
                index: Box::new(var("k")),
            },
            Some(3),
            false,
            "u3",
        ),
    ]);
    let fault = cpp.find("if (volt_fault) {").expect("hata denetimi");
    let guard = cpp.find("if (!volt_port_fits(").expect("koruma");
    assert!(fault < guard);
}

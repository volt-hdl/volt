//! ADR-0095: `volt test` dalga formu testbench'i (§3) ve `volt run`
//! testbench'inde 64 bitten geniş portlar (§5).

use volt_span::FileId;
use volt_sv_emit::{
    collect_sim_ports, find_module, run_testbench_cpp, test_testbench_cpp,
    test_testbench_cpp_traced, SimPort, TbAssertKind, TbStep, TbTest, TbValue,
};

fn port(name: &str, is_input: bool, is_clock: bool, bits: Option<u32>) -> SimPort {
    SimPort {
        name: name.into(),
        is_input,
        is_clock,
        reset: None,
        bits,
    }
}

fn counter_ports() -> Vec<SimPort> {
    vec![
        port("clk", true, true, Some(1)),
        port("enable", true, false, Some(1)),
        port("count", false, false, Some(8)),
    ]
}

fn tests() -> Vec<TbTest> {
    let t = |name: &str, want: u64| TbTest {
        name: name.into(),
        steps: vec![
            TbStep::SetPort {
                port: "enable".into(),
                value: TbValue::Lit(1),
            },
            TbStep::Step(2),
            TbStep::Assert {
                kind: TbAssertKind::Eq,
                left: TbValue::Port("count".into()),
                right: TbValue::Lit(want),
                loc: "counter_test.volt:5".into(),
            },
        ],
    };
    vec![t("a", 2), t("b", 3)]
}

#[test]
fn traced_testbench_opens_one_vcd_per_test_before_the_script() {
    // Arrange
    let vcds = ["waves/a.vcd".to_string(), "waves/b.vcd".to_string()];

    // Act
    let cpp = test_testbench_cpp_traced("Counter", &counter_ports(), &tests(), false, &vcds);

    // Assert
    assert!(cpp.contains("#include \"verilated_vcd_c.h\""), "{cpp}");
    assert!(cpp.contains("static VerilatedVcdC* volt_tfp = nullptr;"));
    assert!(cpp.contains("~VoltTrace() { vcd.close(); volt_tfp = nullptr; }"));
    assert!(cpp.contains("if (volt_tfp) volt_tfp->dump(ctx->time());"));
    assert_eq!(cpp.matches("uctx.traceEverOn(true);").count(), 2);
    assert!(cpp.contains("volt_trace.vcd.open(\"waves/a.vcd\");"));
    assert!(cpp.contains("volt_trace.vcd.open(\"waves/b.vcd\");"));
    // traceEverOn modelden önce, open betikten önce.
    let body = cpp.split("static bool test_0()").nth(1).expect("test_0");
    let ever_on = body.find("traceEverOn").expect("traceEverOn");
    let model = body.find("TOP dut(ctx);").expect("model");
    let open = body.find("vcd.open").expect("open");
    let script = body.find("dut.enable = 1ULL;").expect("betik");
    assert!(ever_on < model && model < open && open < script, "{body}");
    // Betik adımlarının çağrısı değişmez.
    assert!(cpp.contains("run_cycle(&dut, ctx);"));
}

#[test]
fn untraced_testbench_is_unchanged_by_the_tracing_variant() {
    let plain = test_testbench_cpp("Counter", &counter_ports(), &tests());
    assert!(!plain.contains("verilated_vcd_c.h"));
    assert!(!plain.contains("volt_tfp"));
    assert!(!plain.contains("traceEverOn"));
}

#[test]
fn traced_testbench_escapes_backslashes_in_paths() {
    let vcds = ["w\\a.vcd".to_string(), "w/b.vcd".to_string()];
    let cpp = test_testbench_cpp_traced("Counter", &counter_ports(), &tests(), false, &vcds);
    assert!(cpp.contains("vcd.open(\"w/a.vcd\")"), "{cpp}");
}

#[test]
fn run_testbench_drives_and_prints_wide_ports_through_helpers() {
    // Arrange: 128 bitlik giriş, 162 bitlik çıkış, genişliği bilinmeyen çıkış.
    let ports = vec![
        port("clk", true, true, Some(1)),
        port("weight", true, false, Some(128)),
        port("small", true, false, Some(8)),
        port("acc", false, false, Some(162)),
        port("opaque", false, false, None),
    ];

    // Act
    let cpp = run_testbench_cpp("Wide", &ports, 3, None);

    // Assert
    assert!(
        cpp.contains("static void volt_drive(VlWide<N>& port"),
        "{cpp}"
    );
    assert!(cpp.contains("volt_drive(dut.weight, 0);"));
    assert!(cpp.contains("volt_drive(dut.weight, 1);"));
    assert!(!cpp.contains("dut.weight = "));
    // Dar port eski biçimde kalır.
    assert!(cpp.contains("dut.small = 0;"));
    assert!(cpp.contains("(unsigned long long)dut.small"));
    assert!(cpp.contains("volt_show(dut.acc).c_str()"));
    assert!(cpp.contains("volt_show(dut.opaque).c_str()"));
    assert!(!cpp.contains("(unsigned long long)dut.acc"));
    // 162 bit = 6 sözcük: "0x" + 48 basamak sütun genişliği.
    assert!(cpp.contains("%50s"), "{cpp}");
}

#[test]
fn run_testbench_without_wide_ports_has_no_helpers() {
    let cpp = run_testbench_cpp("Counter", &counter_ports(), 3, None);
    assert!(!cpp.contains("volt_drive"));
    assert!(!cpp.contains("volt_show"));
}

#[test]
fn collected_ports_carry_their_width() {
    // Arrange
    let src = "const TN : u8 = 8\n\
               module Top {\n    in clk : clock\n    in w : [Trit; TN * TN]\n    in b : u8\n    out q : bool\n    q = true\n}\n";
    let parsed = volt_syntax::parse(FileId(0), src);

    // Act
    let (ast, module) = find_module(&[&parsed.ast], "Top").expect("Top");
    let ports = collect_sim_ports(ast, module);

    // Assert
    let bits: Vec<(&str, Option<u32>)> = ports.iter().map(|p| (p.name.as_str(), p.bits)).collect();
    assert_eq!(
        bits,
        [
            ("clk", Some(1)),
            ("w", Some(128)),
            ("b", Some(8)),
            ("q", Some(1)),
            // Üretilen SV'deki otomatik reset portu (ADR-0098).
            ("rst", Some(1))
        ]
    );
}

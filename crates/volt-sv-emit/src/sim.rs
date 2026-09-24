//! Verilator C++ testbench üretimi (ADR-0033).
//!
//! İki üreteç var: `run_testbench_cpp` (`volt run` — duman koşusu,
//! çevrim tablosu) ve `test_testbench_cpp` (`volt test` — test bloğu
//! betiğini yürütür, makine-okur `VOLT-*` satırları basar; sürücü bu
//! satırları cargo biçimli rapora çevirir). Üretilen kod İngilizcedir
//! (ADR-0026).

use volt_ast::{ItemKind, ModuleDecl, PortDir, ResetPolarity, SourceFile, TestBinOp, TypeRefKind};

use crate::sim_contract::{CONTRACT_INCLUDES, CONTRACT_PRELUDE};
use crate::sim_script::{
    printf_literal, uses_load, uses_port_check, uses_script_runtime, ScriptEmitter, LOAD_PRELUDE,
    PORT_PRELUDE, SCRIPT_PRELUDE,
};

/// Testbench'in bilmesi gereken port özeti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimPort {
    pub name: String,
    pub is_input: bool,
    pub is_clock: bool,
    /// Reset'i üreteç sürer (ADR-0065); veri gibi sıfırlanmaz/sürülmez.
    pub reset: Option<SimReset>,
}

/// Üretecin sürdüğü reset girişi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimReset {
    /// Ham reset portu (`in r : reset(...)`): etkinleşme polaritesiyle.
    Raw(ResetPolarity),
    /// Ham portlu modülde kalan otomatik `rst`/`rst_n` portu.
    Auto(ResetPolarity),
}

impl SimReset {
    fn polarity(self) -> ResetPolarity {
        match self {
            SimReset::Raw(p) | SimReset::Auto(p) => p,
        }
    }
}

/// Modülün portlarını testbench özetine indirger. Üretilen SV'deki
/// örtük `rst` girişi listeye DAHİL DEĞİLDİR — reset'i üreteçler
/// kendisi sürer. Ham reset portlu modülde (ADR-0065) otomatik reset
/// portları `SimReset::Auto` olarak eklenir: üreteç hepsini adıyla sürer.
pub fn collect_sim_ports(src: &SourceFile, module: &ModuleDecl) -> Vec<SimPort> {
    let mut ports: Vec<SimPort> = module
        .ports
        .iter()
        .map(|p| SimPort {
            name: p.name.text.clone(),
            is_input: p.direction == PortDir::In,
            is_clock: matches!(
                src.types[crate::alias::resolve(src, p.ty)].kind,
                TypeRefKind::Clock
            ),
            reset: None,
        })
        .collect();
    let clocks = crate::clock_ports_of(src, &crate::collect_domains(src), module);
    for (port, sim) in module.ports.iter().zip(ports.iter_mut()) {
        if crate::reset_sync::is_raw_reset(src, port) {
            let polarity = clocks
                .iter()
                .filter_map(|c| c.raw_reset.as_ref())
                .find(|r| r.name == port.name.text)
                .map_or(ResetPolarity::ActiveHigh, |r| r.polarity);
            sim.reset = Some(SimReset::Raw(polarity));
        }
    }
    // Struct portu SV'de yaprak başına bir porttur (ADR-0077): `p_a`, ...
    let ports: Vec<SimPort> = module
        .ports
        .iter()
        .zip(ports)
        .flat_map(|(port, sim)| match struct_leaves(src, port) {
            Some(leaves) => leaves
                .into_iter()
                .map(|suffix| SimPort {
                    name: format!("{}_{suffix}", port.name.text),
                    ..sim.clone()
                })
                .collect(),
            None => vec![sim],
        })
        .collect();
    let mut ports = ports;
    if ports.iter().any(|p| p.reset.is_some()) {
        for cfg in crate::reset_port_set(&clocks) {
            ports.push(SimPort {
                name: cfg.port_name().to_string(),
                is_input: false,
                is_clock: false,
                reset: Some(SimReset::Auto(cfg.polarity)),
            });
        }
    }
    ports
}

/// Struct tipli portun yaprak sonekleri (`a`, `i_x`); struct değilse `None`.
fn struct_leaves(src: &SourceFile, port: &volt_ast::Port) -> Option<Vec<String>> {
    let decl = volt_ast::struct_layout::struct_of_type(src, port.ty)?;
    let layout =
        volt_ast::struct_layout::layout(src, decl, &mut |e| crate::structs::const_int(src, e))
            .ok()?;
    Some(layout.leaves.iter().map(|l| l.suffix()).collect())
}

/// `sources` içinde adı verilen modülü bulur.
pub fn find_module<'a>(
    sources: &[&'a SourceFile],
    name: &str,
) -> Option<(&'a SourceFile, &'a ModuleDecl)> {
    for src in sources {
        for idx in &src.items {
            if let ItemKind::Module(m) = &src.items_arena[*idx].kind {
                if m.name.text == name {
                    return Some((*src, m));
                }
            }
        }
    }
    None
}

/// Bir test betiğinin tek adımı — sürücü, AST `TestStmt`'lerini satır
/// numaralarını çözerek bu biçime indirger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TbStep {
    /// `dut.port = v;` — değerin porta sığdığı derleme zamanında kanıtlı.
    SetPort { port: String, value: TbValue },
    /// `dut.port = <hesaplanmış>;` — değer yazılmadan önce port
    /// genişliğine göre denetlenir; sığmıyorsa test düşer (ADR-0059).
    SetPortChecked {
        port: String,
        value: TbValue,
        check: TbPortCheck,
    },
    /// `step(n);`
    Step(u64),
    /// `reset();`
    Reset,
    /// `assert_*(...)` — `loc` insan-okur konumdur (`dosya.volt:24`).
    Assert {
        kind: TbAssertKind,
        left: TbValue,
        right: TbValue,
        loc: String,
    },
    /// Sonraki deyimlerin kaynak konumu (`dosya.volt:24`) — çalışma
    /// zamanı hatası (indeks, bölme, load) raporu için (ADR-0058).
    Loc(String),
    /// `step(<ifade>);` — çevrim sayısı çalışma zamanında hesaplanır.
    StepBy(TbValue),
    /// `let n = <ifade>;`
    LetScalar { name: String, value: TbValue },
    /// `let a = [..];` / `let a = read_hex("..");` — içerik derleme
    /// zamanında bilinir ve testbench'e gömülür.
    LetArray { name: String, data: Vec<u64> },
    /// `for v in start..end { body }` — çalışma zamanı döngüsü.
    For {
        var: String,
        start: TbValue,
        end: TbValue,
        body: Vec<TbStep>,
    },
    /// `load(dut.mem, source);` — `target` Verilator kök yapısındaki düz
    /// addır (`Soc__DOT__cpu__DOT__imem`); `elem_bits` biliniyorsa
    /// değerler o genişliğe göre denetlenir.
    Load {
        target: String,
        source: String,
        elem_bits: Option<u32>,
    },
}

/// Çalışma zamanı port genişlik denetimi (ADR-0059).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TbPortCheck {
    /// Port genişliği; `None` = derleyici çözemedi, denetim Verilator
    /// modelindeki C++ depolama tipine göre yapılır.
    pub bits: Option<u32>,
    /// İşaretli port aralıktaki negatif sayıyı (`0 - 1`) da kabul eder.
    pub signed: bool,
    /// Raporda görünen tip adı (`u3`); genişlik bilinmiyorsa `?`.
    pub type_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TbAssertKind {
    Eq,
    Ne,
    True,
    False,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TbValue {
    Lit(u64),
    /// `dut.port` okuması.
    Port(String),
    /// Yerel değişken ya da döngü sayacı.
    Var(String),
    /// `len(dizi)`
    Len(String),
    /// `dizi[indeks]` — sınır dışı indeks testi düşürür.
    Index {
        array: String,
        index: Box<TbValue>,
    },
    /// `!x`
    Not(Box<TbValue>),
    Binary {
        op: TestBinOp,
        lhs: Box<TbValue>,
        rhs: Box<TbValue>,
    },
}

/// `load` hedeflerini Verilator'a açan yapılandırma dosyası (`.vlt`):
/// yalnız adı geçen bellekler `public_flat_rw` olur, tasarımın geri
/// kalanı eniyilenmeye devam eder. Girdi: (sahip modül, yazmaç adı).
pub fn load_config_vlt(targets: &[(String, String)]) -> String {
    let mut out =
        String::from("// This file was generated by Volt. DO NOT EDIT.\n`verilator_config\n");
    for (module, var) in targets {
        out.push_str(&format!(
            "public_flat_rw -module \"{module}\" -var \"{var}\"\n"
        ));
    }
    out
}

/// Tek testin adı (görünen ad, boşluksuz) ve adımları.
#[derive(Debug, Clone)]
pub struct TbTest {
    pub name: String,
    pub steps: Vec<TbStep>,
}

/// Saat portlarını bir tam çevrim sürer: posedge + eval, negedge + eval.
/// (`eval()` Verilator modelinin devre değerlendirme API'sidir — kod
/// yürütme değildir.) İzleyiciler açıksa çevrim sayacı posedge'den
/// ÖNCE artar: N. çevrimin kenarında yakalanan ihlal `cycle=N`'dir.
fn cycle_fn(ports: &[SimPort], trace: bool, contracts: bool) -> String {
    let mut set_high = String::new();
    let mut set_low = String::new();
    for p in ports.iter().filter(|p| p.is_clock) {
        set_high.push_str(&format!("    dut->{} = 1;\n", p.name));
        set_low.push_str(&format!("    dut->{} = 0;\n", p.name));
    }
    let dump = if trace {
        "    ctx->timeInc(1);\n    tfp->dump(ctx->time());\n"
    } else {
        "    ctx->timeInc(1);\n"
    };
    let tfp_param = if trace { ", VerilatedVcdC* tfp" } else { "" };
    let count = if contracts { "    ++volt_cycle;\n" } else { "" };
    format!(
        "static void run_cycle(TOP* dut, VerilatedContext* ctx{tfp_param}) {{\n\
         {count}{set_high}    dut->eval();\n{dump}\
         {set_low}    dut->eval();\n{dump}}}\n"
    )
}

/// 2 çevrimlik reset yardımcısı. Ham reset portlu modülde (ADR-0065)
/// bütün reset girişleri polaritesiyle sürülür ve bırakmadan sonra
/// senkronizör zinciri kadar çevrim beklenir: test, bugünkü gibi
/// reset'ten çıkmış bir tasarımla başlar.
fn reset_fn(ports: &[SimPort], trace: bool) -> String {
    let (param, arg) = if trace {
        (", VerilatedVcdC* tfp", ", tfp")
    } else {
        ("", "")
    };
    let resets: Vec<(&str, ResetPolarity)> = ports
        .iter()
        .filter_map(|p| Some((p.name.as_str(), p.reset?.polarity())))
        .collect();
    if resets.is_empty() {
        return format!(
            "static void apply_reset(TOP* dut, VerilatedContext* ctx{param}) {{\n\
             \x20   dut->rst = 1;\n\
             \x20   run_cycle(dut, ctx{arg});\n\
             \x20   run_cycle(dut, ctx{arg});\n\
             \x20   dut->rst = 0;\n\
             }}\n"
        );
    }
    let level =
        |p: ResetPolarity, asserted: bool| u8::from((p == ResetPolarity::ActiveHigh) == asserted);
    let mut out = format!("static void apply_reset(TOP* dut, VerilatedContext* ctx{param}) {{\n");
    for &(name, p) in &resets {
        out.push_str(&format!("    dut->{name} = {};\n", level(p, true)));
    }
    out.push_str(&format!(
        "    run_cycle(dut, ctx{arg});\n    run_cycle(dut, ctx{arg});\n"
    ));
    for &(name, p) in &resets {
        out.push_str(&format!("    dut->{name} = {};\n", level(p, false)));
    }
    out.push_str("    // reset synchronizer release (ADR-0065)\n");
    for _ in 0..crate::reset_sync::RESET_SYNC_STAGES {
        out.push_str(&format!("    run_cycle(dut, ctx{arg});\n"));
    }
    out.push_str("}\n");
    out
}

fn header(module: &str, trace: bool, contracts: bool) -> String {
    let mut out = String::new();
    out.push_str("// This file was generated by Volt. DO NOT EDIT.\n");
    out.push_str(&format!("#include \"V{module}.h\"\n"));
    out.push_str("#include \"verilated.h\"\n");
    if trace {
        out.push_str("#include \"verilated_vcd_c.h\"\n");
    }
    out.push_str("#include <cstdio>\n");
    if contracts {
        out.push_str(CONTRACT_INCLUDES);
    }
    out.push('\n');
    out.push_str(&format!("using TOP = V{module};\n\n"));
    if contracts {
        out.push_str(CONTRACT_PRELUDE);
        out.push('\n');
    }
    out
}

/// `volt run` testbench'i: 2 çevrim reset, `cycle 0` satırı, ardından
/// saat dışı girişler 1'e sürülür ve her çevrimde saat dışı tüm portlar
/// tablo satırı olarak basılır (cli-contract.md §7).
pub fn run_testbench_cpp(
    module: &str,
    ports: &[SimPort],
    cycles: u64,
    vcd_file: Option<&str>,
) -> String {
    run_testbench_cpp_with(module, ports, cycles, vcd_file, false)
}

/// `run_testbench_cpp` + kontrat izleyicileri (ADR-0064, `volt run
/// --contracts`): ihlaller ve cover sayaçları koşu SONUNDA
/// `VOLT-CONTRACT-FAIL` / `VOLT-COVER` satırları olarak basılır —
/// koşu durmaz. `contracts` yalnız SV izleyici içeriyorsa doğru olmalı.
pub fn run_testbench_cpp_with(
    module: &str,
    ports: &[SimPort],
    cycles: u64,
    vcd_file: Option<&str>,
    contracts: bool,
) -> String {
    let trace = vcd_file.is_some();
    let cols: Vec<&SimPort> = ports
        .iter()
        .filter(|p| !p.is_clock && p.reset.is_none())
        .collect();

    let mut out = header(module, trace, contracts);
    out.push_str(&cycle_fn(ports, trace, contracts));
    out.push('\n');
    out.push_str(&reset_fn(ports, trace));
    out.push('\n');

    // Sütun genişlikleri üretim anında sabitlenir (deterministik çıktı).
    let widths: Vec<usize> = cols.iter().map(|p| p.name.len().max(5)).collect();
    let mut head = format!("{:>5}", "cycle");
    let mut dashes = "-----".to_string();
    for (p, w) in cols.iter().zip(&widths) {
        head.push_str(&format!("  {:>w$}", p.name, w = w));
        dashes.push_str(&format!("  {}", "-".repeat(*w)));
    }

    out.push_str("int main(int argc, char** argv) {\n");
    out.push_str("    VerilatedContext ctx;\n");
    out.push_str("    ctx.commandArgs(argc, argv);\n");
    out.push_str("    TOP dut(&ctx);\n");
    if let Some(vcd) = vcd_file {
        out.push_str("    Verilated::traceEverOn(true);\n");
        out.push_str("    VerilatedVcdC vcd;\n");
        out.push_str("    dut.trace(&vcd, 99);\n");
        out.push_str(&format!("    vcd.open(\"{vcd}\");\n"));
    }
    let tfp_arg = if trace { ", &vcd" } else { "" };
    // Girişler resetten önce sıfırlanır (belirsiz başlangıç yok).
    for p in cols.iter().filter(|p| p.is_input) {
        out.push_str(&format!("    dut.{} = 0;\n", p.name));
    }
    out.push_str(&format!("    apply_reset(&dut, &ctx{tfp_arg});\n"));
    if contracts {
        out.push_str("    volt_contracts_begin();\n");
    }
    out.push('\n');
    out.push_str(&format!("    std::printf(\"{head}\\n\");\n"));
    out.push_str(&format!("    std::printf(\"{dashes}\\n\");\n"));

    // Satır basıcı: cycle + saat dışı portlar.
    let mut fmt = "%5llu".to_string();
    let mut args = String::new();
    for (p, w) in cols.iter().zip(&widths) {
        fmt.push_str(&format!("  %{w}llu"));
        args.push_str(&format!(", (unsigned long long)dut.{}", p.name));
    }
    out.push_str(&format!(
        "    std::printf(\"{fmt}\\n\", (unsigned long long)0{args});\n\n"
    ));
    // Duman stimulusu: saat dışı girişler 1 (ADR-0033 — gerçek
    // doğrulama volt test'indir).
    for p in cols.iter().filter(|p| p.is_input) {
        out.push_str(&format!("    dut.{} = 1;\n", p.name));
    }
    out.push_str(&format!(
        "    for (unsigned long long c = 1; c <= {cycles}ULL; ++c) {{\n"
    ));
    out.push_str(&format!("        run_cycle(&dut, &ctx{tfp_arg});\n"));
    out.push_str(&format!("        std::printf(\"{fmt}\\n\", c{args});\n"));
    out.push_str("    }\n");
    if trace {
        out.push_str("    vcd.close();\n");
    }
    out.push_str("    dut.final();\n");
    if contracts {
        out.push_str("    volt_contracts_dump();\n");
        out.push_str("    volt_cover_summary();\n");
    }
    out.push_str("    return 0;\n");
    out.push_str("}\n");
    out
}

/// `volt test` testbench'i: her test taze DUT örneğiyle koşar, sonuç
/// `VOLT-TEST-BEGIN/END` ve ilk hata `VOLT-ASSERT-FAIL` satırlarıyla
/// stdout'a yazılır. Çıkış kodu: kalan test sayısı 0 ise 0, değilse 1.
pub fn test_testbench_cpp(module: &str, ports: &[SimPort], tests: &[TbTest]) -> String {
    test_testbench_cpp_with(module, ports, tests, false)
}

/// `test_testbench_cpp` + kontrat izleyicileri (ADR-0064): her saat
/// ilerleten adımdan sonra ilk ihlal testi düşürür (`VOLT-CONTRACT-FAIL`),
/// cover sayaçları `main` sonunda `VOLT-COVER` satırlarıyla basılır.
/// `contracts` yalnız SV izleyici içeriyorsa doğru olmalı: DPI'sız
/// tasarımda Verilator `svGetScope`'u bağlamaz.
pub fn test_testbench_cpp_with(
    module: &str,
    ports: &[SimPort],
    tests: &[TbTest],
    contracts: bool,
) -> String {
    let script = tests.iter().any(|t| uses_script_runtime(&t.steps));
    let loads = tests.iter().any(|t| uses_load(&t.steps));

    let mut out = header(module, false, contracts);
    if script {
        // <cstddef> ve betik yardımcıları yalnız ADR-0058 özellikleri
        // kullanılırsa eklenir; aksi hâlde çıktı ADR-0033 ile aynıdır.
        out.push_str("#include <cstddef>\n");
        if loads {
            out.push_str(&format!("#include \"V{module}___024root.h\"\n"));
            out.push_str("#include <functional>\n#include <vector>\n");
        }
        out.push('\n');
        out.push_str(SCRIPT_PRELUDE);
        if loads {
            out.push_str(LOAD_PRELUDE);
        }
        out.push('\n');
    }
    if tests.iter().any(|t| uses_port_check(&t.steps)) {
        out.push_str(PORT_PRELUDE);
        out.push('\n');
    }
    out.push_str(&cycle_fn(ports, false, contracts));
    out.push('\n');
    out.push_str(&reset_fn(ports, false));
    out.push('\n');

    for (i, test) in tests.iter().enumerate() {
        // Her test KENDİ VerilatedContext'ini kurar: Verilator, zamanı
        // ilerlemiş bir context'e ikinci model eklenmesine izin vermez.
        out.push_str(&format!("static bool test_{i}() {{\n"));
        out.push_str("    VerilatedContext uctx;\n");
        out.push_str("    VerilatedContext* ctx = &uctx;\n");
        out.push_str("    TOP dut(ctx);\n");
        let test_loads = uses_load(&test.steps);
        if script {
            out.push_str("    volt_fault = 0;\n");
        }
        if test_loads {
            out.push_str("    std::vector<std::function<void()>> volt_loads;\n");
        }
        for p in ports
            .iter()
            .filter(|p| p.is_input && !p.is_clock && p.reset.is_none())
        {
            out.push_str(&format!("    dut.{} = 0;\n", p.name));
        }
        out.push_str("    apply_reset(&dut, ctx);\n");
        if contracts {
            // Çevrim sayacı ve ihlal listesi her testte sıfırdan başlar.
            out.push_str("    volt_contracts_begin();\n");
        }
        let mut emitter = ScriptEmitter::new(test_loads).with_contracts(contracts);
        emitter.emit_steps(&test.steps);
        out.push_str(&emitter.finish());
        out.push_str("    dut.final();\n");
        out.push_str("    return true;\n");
        out.push_str("}\n\n");
    }

    out.push_str("int main(int, char**) {\n");
    out.push_str("    int failed = 0;\n");
    for (i, test) in tests.iter().enumerate() {
        let name = printf_literal(&test.name);
        out.push_str(&format!(
            "    std::printf(\"VOLT-TEST-BEGIN {}\\n\");\n",
            name
        ));
        out.push_str(&format!(
            "    if (test_{i}()) std::printf(\"VOLT-TEST-END {} ok\\n\");\n",
            name
        ));
        out.push_str(&format!(
            "    else {{ ++failed; std::printf(\"VOLT-TEST-END {} fail\\n\"); }}\n",
            name
        ));
    }
    if contracts {
        out.push_str("    volt_cover_summary();\n");
    }
    out.push_str("    return failed == 0 ? 0 : 1;\n");
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter_ports() -> Vec<SimPort> {
        vec![
            SimPort {
                name: "clk".into(),
                is_input: true,
                is_clock: true,
                reset: None,
            },
            SimPort {
                name: "enable".into(),
                is_input: true,
                is_clock: false,
                reset: None,
            },
            SimPort {
                name: "count".into(),
                is_input: false,
                is_clock: false,
                reset: None,
            },
        ]
    }

    #[test]
    fn run_tb_has_table_header_and_smoke_stimulus() {
        let cpp = run_testbench_cpp("Counter", &counter_ports(), 10, None);
        assert!(cpp.contains("#include \"VCounter.h\""));
        assert!(cpp.contains("cycle  enable  count"));
        assert!(cpp.contains("-----  ------  -----"));
        // Reset öncesi girişler 0, duman stimulusu 1.
        assert!(cpp.contains("dut.enable = 0;"));
        assert!(cpp.contains("dut.enable = 1;"));
        assert!(cpp.contains("c <= 10ULL"));
        // VCD kapalıyken trace başlığı yok.
        assert!(!cpp.contains("verilated_vcd_c.h"));
    }

    #[test]
    fn run_tb_vcd_enables_trace() {
        let cpp = run_testbench_cpp("Counter", &counter_ports(), 5, Some("waves.vcd"));
        assert!(cpp.contains("verilated_vcd_c.h"));
        assert!(cpp.contains("vcd.open(\"waves.vcd\")"));
        assert!(cpp.contains("vcd.close()"));
        assert!(cpp.contains("tfp->dump(ctx->time())"));
    }

    #[test]
    fn run_tb_toggles_clock_and_resets_two_cycles() {
        let cpp = run_testbench_cpp("Counter", &counter_ports(), 1, None);
        assert!(cpp.contains("dut->clk = 1;"));
        assert!(cpp.contains("dut->clk = 0;"));
        assert!(cpp.contains("dut->rst = 1;"));
        // apply_reset gövdesinde iki run_cycle çağrısı.
        let reset_body = cpp
            .split("apply_reset")
            .nth(1)
            .expect("apply_reset gövdesi");
        assert_eq!(reset_body.matches("run_cycle").count(), 2);
    }

    #[test]
    fn test_tb_emits_protocol_lines_and_asserts() {
        let tests = vec![TbTest {
            name: "start_bit_is_low".into(),
            steps: vec![
                TbStep::SetPort {
                    port: "enable".into(),
                    value: TbValue::Lit(1),
                },
                TbStep::Step(2),
                TbStep::Assert {
                    kind: TbAssertKind::Eq,
                    left: TbValue::Port("count".into()),
                    right: TbValue::Lit(2),
                    loc: "counter_test.volt:5".into(),
                },
                TbStep::Reset,
                TbStep::Assert {
                    kind: TbAssertKind::False,
                    left: TbValue::Port("count".into()),
                    right: TbValue::Lit(0),
                    loc: "counter_test.volt:7".into(),
                },
            ],
        }];
        let cpp = test_testbench_cpp("Counter", &counter_ports(), &tests);
        assert!(cpp.contains("VOLT-TEST-BEGIN start_bit_is_low"));
        assert!(cpp.contains("VOLT-TEST-END start_bit_is_low ok"));
        assert!(cpp.contains("VOLT-ASSERT-FAIL assert_eq counter_test.volt:5"));
        assert!(cpp.contains("((unsigned long long)dut.count) == (2ULL)"));
        assert!(cpp.contains("((unsigned long long)dut.count) == 0"));
        // Her test taze DUT kurar ve reset uygular.
        assert!(cpp.contains("static bool test_0()"));
        assert!(cpp.contains("VerilatedContext uctx;"));
        assert_eq!(cpp.matches("apply_reset(&dut, ctx);").count(), 2);
        assert!(cpp.contains("return failed == 0 ? 0 : 1;"));
    }

    #[test]
    fn test_tb_fresh_dut_per_test() {
        let mk = |name: &str| TbTest {
            name: name.into(),
            steps: vec![TbStep::Step(1)],
        };
        let cpp = test_testbench_cpp("Counter", &counter_ports(), &[mk("a"), mk("b")]);
        assert!(cpp.contains("test_0"));
        assert!(cpp.contains("test_1"));
        assert_eq!(cpp.matches("TOP dut(ctx);").count(), 2);
    }
}

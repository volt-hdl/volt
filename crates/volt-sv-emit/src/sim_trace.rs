//! Testbench iz (VCD) parçaları: `volt run --vcd` ve `volt test` dalga
//! formu (ADR-0092, ADR-0095 §3).

/// İz (VCD) dökümü nereden yapılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trace {
    Off,
    /// `volt run --vcd`: `run_cycle`'a `tfp` parametresi.
    Param,
    /// `volt test` dalga formu (ADR-0095): koşan testin `volt_tfp`'si —
    /// betik adımlarının `run_cycle(&dut, ctx)` çağrısı değişmez.
    Global,
}

/// İzli `volt test` testbench'inin ön bildirimi (ADR-0095 §3): `VoltTrace`
/// testin sonunda (erken `return false` dahil) VCD'yi kapatır.
pub(crate) const TRACE_PRELUDE: &str = "\
// Waveform of the running test (ADR-0095): run_cycle dumps while set.
static VerilatedVcdC* volt_tfp = nullptr;
struct VoltTrace {
    VerilatedVcdC vcd;
    ~VoltTrace() { vcd.close(); volt_tfp = nullptr; }
};
";

/// Test fonksiyonunda modeli kuran satırlar; `vcd` verilirse iz, model
/// kurulmadan ÖNCE açılır (Verilator kuralı) ve dosya betikten önce açılır.
/// Yol testbench'e `/` ayraçlı gömülür.
pub(crate) fn model_lines(vcd: Option<&str>) -> String {
    let Some(vcd) = vcd else {
        return "    TOP dut(ctx);\n".to_string();
    };
    let path = vcd.replace('\\', "/").replace('"', "\\\"");
    format!(
        "    uctx.traceEverOn(true);\n\
         \x20   TOP dut(ctx);\n\
         \x20   VoltTrace volt_trace;\n\
         \x20   dut.trace(&volt_trace.vcd, 99);\n\
         \x20   volt_trace.vcd.open(\"{path}\");\n\
         \x20   volt_tfp = &volt_trace.vcd;\n"
    )
}

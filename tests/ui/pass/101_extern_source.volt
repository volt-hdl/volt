// ADR-0076: an extern module names its SystemVerilog body with @source;
// the path is relative to this file and stays inside the project.
// 'volt build' emits only the instantiation, 'volt run'/'test'/'verify'
// also hand rtl/101_ext_delay.sv to Verilator / SymbiYosys.
@source("rtl/101_ext_delay.sv")
extern module ExtDelay {
    in  clk : clock
    in  d   : u8
    out q   : u8
}

module Top {
    in  clk : clock
    in  d   : u8
    out q   : u8
    let r = ExtDelay { clk: clk, d: d }
    q = r.q
}

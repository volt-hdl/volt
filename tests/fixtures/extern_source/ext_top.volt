// Extern modules with their SystemVerilog source (ADR-0076). The bodies
// live in rtl/ext_ops.sv; 'volt test' hands the file to Verilator and
// 'volt verify' to SymbiYosys. The invariant below holds only because
// ExtInvert really inverts -- it cannot be proven without the body.

@source("rtl/ext_ops.sv")
extern module ExtInvert {
    in  a : u8
    out y : u8
}

@source("rtl/ext_ops.sv")
extern module ExtDelay {
    in  clk : clock
    in  d   : u8
    out q   : u8
}

module ExtTop {
    in  clk : clock
    in  d   : u8
    out inv : u8
    out dly : u8

    invariant: (inv ^ d) == 255

    let i = ExtInvert { a: d }
    let r = ExtDelay { clk: clk, d: i.y }
    inv = i.y
    dly = r.q
}

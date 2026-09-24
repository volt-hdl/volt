//~ E1012
// ADR-0076: @source names a file that does not exist next to this file.
@source("rtl/99_no_such_file.sv")
//~^ ERROR cannot read SystemVerilog source
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

//~ E1013
// ADR-0078: a register becomes an SV 'logic' of the same name.
module Lut {
    in  clk : clock
    in  d   : u8
    out q   : u8

    reg table : u8 = 0
//~^ ERROR 'table' is a SystemVerilog keyword
    on clk { table <= d }
    q = table
}

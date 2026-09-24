//~ E1013
// ADR-0078: a module-level let becomes an SV wire of the same name.
module Stamp {
    in  a : u8
    out y : u8

    let time : u8 = a + 1
//~^ ERROR 'time' is a SystemVerilog keyword
    y = time
}

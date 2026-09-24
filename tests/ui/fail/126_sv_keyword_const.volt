//~ E1013
// ADR-0078: a constant array can become a localparam of the same name
// (ADR-0041 ConstArrayStyle); every constant name is checked.
const table : [u8; 2] = [1, 2]
//~^ ERROR 'table' is a SystemVerilog keyword

module T {
    in  a : bool
    out y : u8
    y = table[a as u1]
}

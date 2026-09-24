//~ E1013
// ADR-0078: a wire keeps its name in SystemVerilog.
module Edge {
    in  a : u8
    out y : u8

    wire edge : u8
//~^ ERROR 'edge' is a SystemVerilog keyword
    edge = a
    y = edge
}

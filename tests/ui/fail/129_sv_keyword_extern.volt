//~ E1013
// ADR-0078: an extern module's ports are connected by name ('.cell(a)');
// a keyword port could not even be declared in the SV body.
extern module Ext {
    in  cell : u8
//~^ ERROR 'cell' is a SystemVerilog keyword
    out y    : u8
}

module Top {
    in  a : u8
    out y : u8
    let u = Ext { cell: a }
    y = u.y
}

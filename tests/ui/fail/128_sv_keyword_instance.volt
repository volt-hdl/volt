//~ E1013
// ADR-0078: a user-module instance keeps its name in SystemVerilog
// ('Sub instance (...)').
module Sub {
    in  a : u8
    out y : u8
    y = a
}

module Top {
    in  a : u8
    out y : u8
    let instance = Sub { a: a }
//~^ ERROR 'instance' is a SystemVerilog keyword
    y = instance.y
}

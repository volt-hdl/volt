//~ W1006
// #143: a generic module that nothing instantiates is a template. Its
// parameter has no value, so no SystemVerilog is generated for it, and the
// compiler says so. The non-generic module beside it is generated as
// before. (Before #143 the template was written with N resolved to a
// same-named root const, or left unresolved.)

module Fir<const N: u32> {
//~^ ERROR module 'Fir' is generic and is never instantiated
    out y : u8
    y = N
}

module Top {
    in  a : u8
    out b : u8
    b = a
}

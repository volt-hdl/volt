//~ E1013
// ADR-0078: the module name is the SV module name and file name.
module config {
//~^ ERROR 'config' is a SystemVerilog keyword
    in  a : u8
    out y : u8
    y = a
}

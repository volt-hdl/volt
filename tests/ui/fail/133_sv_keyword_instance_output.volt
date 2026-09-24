//~ E1013
// ADR-0078: an instance output is read through a '<instance>_<port>' wire.
module Sub {
    in  a        : u8
    out ondetect : u8
    ondetect = a
}

module T {
    in  a : u8
    out y : u8
    let pulsestyle = Sub { a: a }
//~^ ERROR 'pulsestyle' becomes the SystemVerilog name 'pulsestyle_ondetect'
    y = pulsestyle.ondetect
}

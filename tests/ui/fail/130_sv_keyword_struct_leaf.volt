//~ E1013
// ADR-0078: struct fields become '<signal>_<field>' signals (ADR-0077);
// 'pulsestyle' + 'ondetect' is the keyword 'pulsestyle_ondetect'.
struct Mode {
    ondetect : u8
}

module T {
    in  pulsestyle : Mode
//~^ ERROR 'pulsestyle' becomes the SystemVerilog name 'pulsestyle_ondetect'
    out y : u8
    y = pulsestyle.ondetect
}

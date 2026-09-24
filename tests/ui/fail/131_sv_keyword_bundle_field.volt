//~ E1013
// ADR-0078: bundle ports flatten to '<port>_<field>' (ADR-0039).
struct port Pulse {
    out onevent : u8
}

module T {
    out pulsestyle : Pulse
//~^ ERROR 'pulsestyle' becomes the SystemVerilog name 'pulsestyle_onevent'
    pulsestyle.onevent = 1
}

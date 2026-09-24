//~ E1013
// ADR-0078: enum variants become '<Enum>_<Variant>' localparams (ADR-0074).
enum pulsestyle {
    ondetect,
//~^ ERROR 'ondetect' becomes the SystemVerilog name 'pulsestyle_ondetect'
    onevent,
}

module T {
    in  clk : clock
    in  a   : bool
    out y   : bool
    reg s : pulsestyle = pulsestyle::ondetect
    on clk { if a { s <= pulsestyle::onevent } }
    y = s == pulsestyle::onevent
}

//~ PASS
// `use` loads ./periph.volt (ADR-0042) for one fn; the modules there are
// never instantiated, so only Top is emitted and verified.
use periph::low_nibble;

module Top {
    in  clk : clock
    in  a   : u8
    out y   : u8
    reg r : u8 = 0
    on clk { r <= low_nibble(a) }
    y = r
    invariant: r < 16
}

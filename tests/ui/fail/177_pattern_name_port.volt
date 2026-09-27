//~ E1015
// ADR-0085: a name in a pattern is a value. A port changes every cycle,
// it cannot be a case label — and it must not silently bind a new
// variable that catches every value.

module PortPattern {
    in  clk : clock
    in  x   : u8
    in  lim : u8
    out hit : bool

    reg hit_r : bool = false
    on clk {
        match x {
            lim => { hit_r <= true }
            //~^ ERROR 'lim' in a pattern must be a constant, but it is a port
            _ => { hit_r <= false }
        }
    }
    hit = hit_r
}

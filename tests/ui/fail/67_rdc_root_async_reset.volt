//~ W3009
// Nothing in the unit instantiates Top, so nothing in Volt synchronizes
// the release of its automatic asynchronous reset (ADR-0065 R1).
domain Core {
    clock = posedge
    reset = async active_high
}

module Top {
    in  clk : clock @Core
    //~^ ERROR W3009
    in  d   : u8
    out q   : u8

    reg r : u8 = 0
    on clk {
        r <= d
    }
    q = r
}

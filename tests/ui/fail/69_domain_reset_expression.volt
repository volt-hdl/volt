//~ E0001
// A domain reset is a property, not a signal (ADR-0065 R4): an
// expression used to be ignored silently, now it is a syntax error.
domain Core {
    clock = posedge
    reset = rst_in
    //~^ ERROR E0001
}

module Top {
    in  clk : clock @Core
    in  d   : u8
    out q   : u8

    reg r : u8 = 0
    on clk {
        r <= d
    }
    q = r
}

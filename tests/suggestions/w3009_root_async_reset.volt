// suggestion: W3009
// Nothing instantiates Top, so nothing synchronizes the release of its
// automatic asynchronous reset: the fix declares the raw reset port.
domain Core {
    clock = posedge
    reset = async active_high
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

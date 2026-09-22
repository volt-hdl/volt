// ADR-0065 sec. 1: only the top takes the raw reset; the child keeps its
// automatic rst_n port and receives the parent's synchronized reset.
// One chain per (clock, raw reset): no convergence, no W3009.
domain Core {
    clock = posedge
    reset = async active_low
}

module Child {
    in  clk : clock @Core
    in  d   : u8
    out q   : u8

    reg r : u8 = 0
    on clk {
        r <= d
    }
    q = r
}

module Top {
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q     : u8

    let u = Child {
        clk: clk,
        d: d,
    }
    q = u.q
}

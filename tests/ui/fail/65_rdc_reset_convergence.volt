//~ E3003
// The same raw reset is synchronized twice on clk: once in Top and
// again inside the instance (ADR-0065 R6). The two chains can release
// in different cycles.
domain Core {
    clock = posedge
    reset = async active_low
}

module Child {
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q     : u8

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
    out q2    : u8

    reg r : u8 = 0
    on clk {
        r <= d
    }
    q = r
    let u = Child {
        clk: clk,
        rst_n: rst_n,
        //~^ ERROR E3003
        d: d,
    }
    q2 = u.q
}

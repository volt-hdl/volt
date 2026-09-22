//~ E3003
// The raw port feeds only @Fast; @Slow keeps an automatic rst_n port,
// so the generated module would have two inputs called rst_n
// (ADR-0065 sec. 1).
domain Fast {
    clock = posedge
    reset = async active_low
}

domain Slow {
    clock = posedge
    reset = async active_low
}

module Top {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  rst_n    : reset(async, active_low) @Fast
    //~^ ERROR E3003
    in  a        : u8 @Fast
    out qa       : u8 @Fast
    out qb       : u8 @Slow

    reg(fast_clk) ra : u8 = 0
    reg(slow_clk) rb : u8 = 0
    on fast_clk {
        ra <= a
    }
    on slow_clk {
        rb <= rb + 1
    }
    qa = ra
    qb = rb
}

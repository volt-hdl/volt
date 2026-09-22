//~ E3010
// Two raw reset ports without domain annotations (ADR-0065 sec. 1): it
// is ambiguous which domain each one resets.
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
    in  rst_a    : reset(async, active_low)
    //~^ ERROR E3010
    in  rst_b    : reset(async, active_low)
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

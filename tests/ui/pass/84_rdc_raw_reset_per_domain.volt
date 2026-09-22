// ADR-0065 sec. 1: with more than one raw reset port each one names the
// domain it resets; kinds may differ per domain.
domain Fast {
    clock = posedge
    reset = async active_low
}

domain Slow {
    clock = negedge
    reset = sync active_high
}

module Top {
    in  fast_clk   : clock @Fast
    in  slow_clk   : clock @Slow
    in  fast_rst_n : reset(async, active_low) @Fast
    in  slow_rst   : reset(sync, active_high) @Slow
    in  a          : u8 @Fast
    out qa         : u8 @Fast
    out qb         : u8 @Slow

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

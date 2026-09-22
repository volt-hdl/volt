//~ E3003
// Two asynchronous domains with the same polarity share the one
// generated reset port rst_n (ADR-0065 R5): its release can be
// synchronous to at most one of the two clocks.
domain Fast {
    clock = posedge
    reset = async active_low
}

domain Slow {
    clock = posedge
    reset = async active_low
}

module Bridge {
    in  fast_clk  : clock @Fast
    //~^ ERROR E3003
    in  slow_clk  : clock @Slow
    in  fast_in   : bool  @Fast
    out fast_seen : bool  @Fast
    out slow_out  : bool  @Slow

    reg(fast_clk) seen : bool = false
    on fast_clk {
        seen <= fast_in
    }
    fast_seen = seen
    slow_out = sync(fast_in, slow_clk)
}

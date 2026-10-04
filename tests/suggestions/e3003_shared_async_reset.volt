// suggestion: E3003
// Two asynchronous domains share the generated reset port rst_n: the fix
// declares the raw port, and each clock gets its own reset synchronizer.
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

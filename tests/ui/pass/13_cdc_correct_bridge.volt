// Correct CDC bridge -- crossing via sync()
// domain-inference.md K9
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module CdcBridge {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow

    // Two-flop synchronization for a single-bit signal
    slow_flag = sync(fast_flag, slow_clk)
}

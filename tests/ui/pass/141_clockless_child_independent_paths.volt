// ADR-0114 K2: the module summary is per output port. Two independent
// paths through one clockless module may carry two domains: y1 = a1 is
// @Fast and crosses through sync(), y2 = a2 stays @Slow.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module Two {
    in  a1 : bool
    in  a2 : bool
    out y1 : bool
    out y2 : bool
    y1 = a1
    y2 = a2
}

module Bridge {
    in  _fast_clk : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    in  slow_in   : bool  @Slow
    out slow_flag : bool  @Slow
    out slow_echo : bool  @Slow

    let t = Two { a1: fast_flag, a2: slow_in }
    let ty1 = t.y1
    slow_flag = sync(ty1, slow_clk)
    slow_echo = t.y2
}

// Output net (ADR-0079):
//~ LINT-ALLOW: UNUSEDSIGNAL: _fast_clk declares the @Fast domain; sync() of a let has no capture stage in the source domain (#198)

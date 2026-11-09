// ADR-0114: an output that reads no input is a constant (Timeless) and
// fits every domain, although the instance is bound to a @Fast input.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module K {
    in  a : bool
    out y : bool
    out k : bool
    y = a
    k = true
}

module Bridge {
    in  _fast_clk : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow
    out slow_one  : bool  @Slow

    let k = K { a: fast_flag }
    let ky = k.y
    slow_flag = sync(ky, slow_clk)
    slow_one = k.k
}

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

module ConstOut {
    in  a   : bool
    out y   : bool
    out one : bool
    y   = a
    one = true
}

module Bridge {
    in  _fast_clk : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow
    out slow_one  : bool  @Slow

    let c = ConstOut { a: fast_flag }
    let cy = c.y
    slow_flag = sync(cy, slow_clk)
    slow_one = c.one
}

// Output net (ADR-0079):
//~ LINT-ALLOW: UNUSEDSIGNAL: _fast_clk declares the @Fast domain; sync() of a let has no capture stage in the source domain (#198)

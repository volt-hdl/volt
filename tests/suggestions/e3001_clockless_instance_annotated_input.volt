// suggestion: E3001
// A clockless module joins two domains in one output, and the input that
// conflicts carries its own @Slow annotation (ADR-0114 K3, K8b). The value
// bound to an annotated input must stay in that domain (K8 step 2), so the
// fix synchronizes the other input, fast_flag, into @Slow above the
// instance and binds the synchronized name.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module And2 {
    in  a : bool
    in  b : bool @Slow
    out y : bool
    y = a && b
}

module Merge {
    in  _fast_clk : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    in  slow_in   : bool  @Slow
    out slow_out  : bool  @Slow

    let g = And2 { a: fast_flag, b: slow_in }
    slow_out = g.y
}

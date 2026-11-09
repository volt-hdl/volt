// suggestion: E3001
// A clockless module joins a @Fast and a @Slow input in one output
// (ADR-0114 K3): the fix synchronizes the second input into the first
// one's domain above the instance and binds the synchronized name.
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
    in  b : bool
    out y : bool
    y = a && b
}

module Merge {
    in  fast_clk  : clock @Fast
    in  _slow_clk : clock @Slow
    in  fast_flag : bool  @Fast
    in  slow_in   : bool  @Slow
    out fast_out  : bool  @Fast

    let g = And2 { a: fast_flag, b: slow_in }
    fast_out = g.y
}

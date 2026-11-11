// suggestion: E3001
// #204: the synchronizer fix on a shorthand port binding keeps the port
// name: `And2 { fast_flag, slow_in }` binds `slow_in: <synchronized>`.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module And2 {
    in  fast_flag : bool
    in  slow_in   : bool
    out y         : bool
    y = fast_flag && slow_in
}

module Merge {
    in  fast_clk  : clock @Fast
    in  _slow_clk : clock @Slow
    in  fast_flag : bool  @Fast
    in  slow_in   : bool  @Slow
    out fast_out  : bool  @Fast

    let g = And2 { fast_flag, slow_in }
    fast_out = g.y
}

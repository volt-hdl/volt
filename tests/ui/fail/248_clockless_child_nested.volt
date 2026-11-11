//~ E3001
// ADR-0114: summaries are built from the child up, so a clockless module
// inside a clockless module still carries its input's domain.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module Pass {
    in  a : bool
    out y : bool
    y = a
}

module Outer {
    in  a : bool
    out y : bool
    let p = Pass { a: a }
    y = !p.y
}

module CdcBridge {
    in  _fast_clk : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow

    let o = Outer { a: fast_flag }
    slow_flag = o.y
    //~^ ERROR direct assignment between clock domains
}

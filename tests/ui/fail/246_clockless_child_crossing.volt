//~ E3001
// #189, ADR-0114 (domain-inference.md K8b): a module without a clock is
// combinational; its output carries the domain of the inputs it depends
// on. p.y is @Fast here, so assigning it to a @Slow output is a crossing
// without a synchronizer.
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

module CdcBridge {
    in  _fast_clk : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow

    let p = Pass { a: fast_flag }
    slow_flag = p.y
    //~^ ERROR direct assignment between clock domains
}

//~ E3001
// ADR-0114 (domain-inference.md K8b): an annotated output of a module
// without a clock keeps its own domain, but the inputs it depends on must
// carry that domain. P.y is declared @Fast and follows a, and a is bound
// to a @Slow value: the value reaches the @Fast side without a
// synchronizer.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module P {
    in  a : bool
    out y : bool @Fast
    y = a
}

module Top {
    in  _fast_clk : clock @Fast
    in  _slow_clk : clock @Slow
    in  slow_flag : bool  @Slow
    out fast_out  : bool  @Fast

    let p = P { a: slow_flag }
    //~^ ERROR drives 'P.y', declared @Fast, from @Slow
    fast_out = p.y
}

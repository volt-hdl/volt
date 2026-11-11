// suggestion: E3001
// An annotated output of a clockless module is fed from another domain
// (ADR-0114, domain-inference.md K8b): the fix synchronizes the input
// into the output's domain above the instance and binds the synchronized
// name.
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
    in  fast_clk  : clock @Fast
    in  _slow_clk : clock @Slow
    in  slow_flag : bool  @Slow
    out fast_out  : bool  @Fast

    let p = P { a: slow_flag }
    fast_out = p.y
}

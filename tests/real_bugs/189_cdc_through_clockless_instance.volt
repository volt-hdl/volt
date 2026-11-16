//~ E3001
// #189: a clock-domain crossing through an instance of a module without a
// clock was accepted. The ports of the clockless child were Timeless, so
// `p.y` carried no domain and `slow_flag = p.y` passed the K6 check; the
// SystemVerilog had a combinational path from @Fast to @Slow with no
// synchronizer. domain-inference.md K5 (a combinational value has the join
// of its operands' domains), K6 (target and source domains must match,
// else E3001) and K8b / ADR-0114 (the output of a clockless child carries
// the domain of the inputs it depends on): p.y is @Fast, E3001. Fixed in
// PR #200.
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
    in  _slow_clk : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow

    let p = Pass { a: fast_flag }
    slow_flag = p.y
}

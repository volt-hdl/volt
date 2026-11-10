// ADR-0114 (domain-inference.md K8b): an annotated output of a module
// without a clock keeps its own domain when the inputs it depends on
// carry that domain. P.y is declared @Fast and follows a; a is bound to a
// @Fast value. P.k reads no input: a constant fits every domain. The
// @Slow input is synchronized before it enters the module.
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
    in  b : bool
    out y : bool @Fast
    out z : bool @Fast
    out k : bool @Slow
    y = a
    z = b
    k = true
}

module Top {
    in  fast_clk  : clock @Fast
    in  _slow_clk : clock @Slow
    in  fast_flag : bool  @Fast
    in  slow_flag : bool  @Slow
    out fast_out  : bool  @Fast
    out fast_echo : bool  @Fast
    out slow_out  : bool  @Slow

    let slow_flag_sync = sync(slow_flag, fast_clk)
    let p = P { a: fast_flag, b: slow_flag_sync }
    fast_out = p.y
    fast_echo = p.z
    slow_out = p.k
}

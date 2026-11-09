// ADR-0114: the output of a clockless child carries its input's domain
// (@Fast); naming it with 'let' and passing it through sync() is the
// legal crossing (K9). This is the form the E3001 fix-it writes; the
// name avoids 'p_y', the generated name of the instance output (ADR-0090).
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

module Bridge {
    in  _fast_clk : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow

    let p = Pass { a: fast_flag }
    let p_y_raw = p.y
    slow_flag = sync(p_y_raw, slow_clk)
}

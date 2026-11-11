// suggestion: E3001
// The fix names the instance output before sync(). 'p_y_raw' and
// 'p_y_raw_2' are the generated names of p.y_raw and p.y_raw_2 (ADR-0090),
// so the new name is 'p_y_raw_3': a 'let' with a taken name would be E1003.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module Pass3 {
    in  a       : bool
    out y       : bool
    out y_raw   : bool
    out y_raw_2 : bool
    y       = a
    y_raw   = !a
    y_raw_2 = a
}

module Cross {
    in  _fclk : clock @Fast
    in  sclk  : clock @Slow
    in  a     : bool  @Fast
    out y     : bool  @Slow

    let p = Pass3 { a: a }
    y = p.y
}

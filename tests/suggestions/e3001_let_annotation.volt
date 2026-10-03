// suggestion: E3001
// A let annotated with a domain takes a value of another domain.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module Cross {
    in  _fclk : clock @Fast
    in  sclk  : clock @Slow
    in  a     : bool  @Fast
    out y     : bool  @Slow

    let s @Slow = a
    y = s
}

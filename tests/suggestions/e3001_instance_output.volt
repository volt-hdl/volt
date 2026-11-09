// suggestion: E3001
// The output of a clockless child carries its input's domain (ADR-0114):
// sync() cannot take 'p.y' directly (E0003), so the fix names it with a
// 'let' first and synchronizes that name.
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

module Cross {
    in  _fclk : clock @Fast
    in  sclk  : clock @Slow
    in  a     : bool  @Fast
    out y     : bool  @Slow

    let p = Pass { a: a }
    y = p.y
}

// suggestion: E3001
// A one-bit signal crosses into another clock domain at module level:
// the fix wraps the source in sync() with the destination clock.
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

    y = a
}

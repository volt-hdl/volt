// suggestion: E3001
// The crossing is inside a 'comb' block: same fix as in an 'on' block.
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

    comb {
        y = a
    }
}

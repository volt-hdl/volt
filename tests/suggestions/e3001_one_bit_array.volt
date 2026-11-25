// suggestion: E3001
// A one-element bool array is one bit (ADR-0056, #419: the width comes
// from one function), so the crossing gets the same sync() fix as a bool.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module Cross {
    in  _fclk : clock     @Fast
    in  sclk  : clock     @Slow
    in  a     : [bool; 1] @Fast
    out y     : [bool; 1] @Slow

    y = a
}

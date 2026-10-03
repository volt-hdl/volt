// suggestion: E3012
// An 'on' block's condition reads a foreign domain: the fix synchronizes
// it at module level and reads the synchronized name.
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
    in  go    : bool  @Fast
    out y     : u8    @Slow

    reg n : u8 @Slow = 0
    on sclk {
        if go {
            n <= n + 1
        }
    }
    y = n
}

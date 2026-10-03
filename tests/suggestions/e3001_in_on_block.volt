// suggestion: E3001
// The crossing is inside an 'on' block, where sync() cannot be written:
// the fix adds a module-level 'let ... = sync(...)' above the block and
// reads that name in the block.
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

    reg r : bool @Slow = false
    on sclk {
        r <= a
    }
    y = r
}

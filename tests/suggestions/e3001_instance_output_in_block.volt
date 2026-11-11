// suggestion: E3001
// Same crossing inside an 'on' block, where sync() cannot be written:
// the fix adds the naming 'let' and the 'let ... = sync(...)' above the
// block and reads the synchronized name in the block.
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
    reg r : bool @Slow = false
    on sclk {
        r <= p.y
    }
    y = r
}

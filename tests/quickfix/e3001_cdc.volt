// no-quickfix: E3001
// Crossing a clock domain needs a choice (sync, AsyncFifo, ...).
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module Cross {
    in  fclk : clock @Fast
    in  sclk : clock @Slow
    in  a    : u8    @Fast
    out y    : u8    @Slow

    y = a
}

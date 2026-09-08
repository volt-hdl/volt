// Built-in AsyncFifo primitive (ADR-0027) -- multi-bit data crossing
// between two clock domains via gray-coded pointers. The write side
// lives in @Fast, the read side in @Slow; the compiler generates the
// dual-clock FIFO body and its formal contracts.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module FifoBridge {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  din      : u8    @Fast
    in  push     : bool  @Fast
    in  pop      : bool  @Slow
    out dout     : u8    @Slow
    out full     : bool  @Fast
    out empty    : bool  @Slow

    let u_fifo = AsyncFifo<u8, 16> {
        wr_clk: fast_clk,
        wr_data: din,
        wr_en: push,
        rd_clk: slow_clk,
        rd_en: pop,
    }

    // Outputs are read back with field access.
    full  = u_fifo.wr_full
    dout  = u_fifo.rd_data
    empty = u_fifo.rd_empty
}

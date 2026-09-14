// Built-in AsyncDualPortRam primitive (ADR-0049) -- a true dual-clock
// memory: the write port lives in @Fast, the read port in @Slow. No
// pointer synchronization is needed because every address and data
// signal stays in its own domain; the memory array itself is the CDC
// boundary. W3006 reminds that a read of an address being written
// from the other clock returns an undefined value.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module DualClockBuffer {
    in  fast_clk : clock   @Fast
    in  slow_clk : clock   @Slow
    in  wr_addr  : bits<8> @Fast
    in  wr_data  : u16     @Fast
    in  wr_en    : bool    @Fast
    in  rd_addr  : bits<8> @Slow
    out rd_data  : u16     @Slow   // one slow_clk after rd_addr

    let buf = AsyncDualPortRam<u16, 256> {
        wr_clk:  fast_clk,
        wr_addr: wr_addr,
        wr_data: wr_data,
        wr_en:   wr_en,
        rd_clk:  slow_clk,
        rd_addr: rd_addr,
    }

    // The read register is in the read domain: no sync() needed.
    rd_data = buf.rd_data
}

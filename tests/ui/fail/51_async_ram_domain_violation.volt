//~ E3001
// AsyncDualPortRam wired wrong (ADR-0049): rd_addr belongs to the read
// side (@Dst, bound to Slow by rd_clk) but the address comes from Fast.
// Each port of a dual-clock memory keeps its own domain; carrying an
// address across is exactly the mistake the primitive exists to prevent.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module Bad {
    in  fast_clk : clock   @Fast
    in  slow_clk : clock   @Slow
    in  wr_addr  : bits<8> @Fast
    in  wr_data  : u16     @Fast
    in  wr_en    : bool    @Fast
    in  fast_ra  : bits<8> @Fast
    out rd_data  : u16     @Slow

    let buf = AsyncDualPortRam<u16, 256> {
        wr_clk:  fast_clk,
        wr_addr: wr_addr,
        wr_data: wr_data,
        wr_en:   wr_en,
        rd_clk:  slow_clk,
        rd_addr: fast_ra,
        //~^ ERROR E3001
    }
    rd_data = buf.rd_data
}

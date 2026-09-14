//~ E3001
// Extern CDC primitive wired wrong (ADR-0047): wr_data belongs to the
// write side (@Src, bound to SysDomain by wr_clk) but the value comes
// from PixDomain. Before ADR-0047 the compiler was silent here.
domain SysDomain {
    clock = posedge
    reset = sync active_high
}

domain PixDomain {
    clock = posedge
    reset = sync active_high
}

extern module ExtAsyncFifo {
    in  wr_clk   : clock @Src
    in  wr_data  : u8    @Src
    in  wr_en    : bool  @Src
    out wr_full  : bool  @Src

    in  rd_clk   : clock @Dst
    out rd_data  : u8    @Dst
    in  rd_en    : bool  @Dst
    out rd_empty : bool  @Dst
}

module Bad {
    in  sys_clk  : clock @SysDomain
    in  sys_en   : bool  @SysDomain
    out sys_full : bool  @SysDomain

    in  pix_clk  : clock @PixDomain
    in  pix_d    : u8    @PixDomain
    in  pix_en   : bool  @PixDomain
    out pix_q    : u8    @PixDomain

    let f = ExtAsyncFifo {
        wr_clk:  sys_clk,
        wr_data: pix_d,
        //~^ ERROR E3001
        wr_en:   sys_en,
        rd_clk:  pix_clk,
        rd_en:   pix_en,
    }
    sys_full = f.wr_full
    pix_q    = f.rd_data
}

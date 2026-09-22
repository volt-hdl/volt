// ADR-0065 R5': the write side of an AsyncDualPortRam is a memory array
// without reset (ADR-0049), so a clock that only drives it samples no
// reset. FrameStore resets on pix_clk alone (the read register): no
// W3010, and the parent connects the pix_clk chain to its rst port --
// not the sys_clk one, even though sys_clk comes first. W3006 (read of
// an address being written) is the expected ADR-0049 reminder.
domain Sys {
    clock = posedge
}

domain Pix {
    clock = posedge
}

module FrameStore {
    in  sys_clk : clock    @Sys
    in  wr_addr : bits<6>  @Sys
    in  wr_data : bool     @Sys
    in  wr_en   : bool     @Sys
    in  pix_clk : clock    @Pix
    in  rd_addr : bits<6>  @Pix
    out rd_data : bool     @Pix

    let mem = AsyncDualPortRam<bool, 64> {
        wr_clk:  sys_clk,
        wr_addr: wr_addr,
        wr_data: wr_data,
        wr_en:   wr_en,
        rd_clk:  pix_clk,
        rd_addr: rd_addr,
    }
    rd_data = mem.rd_data
}

module Display {
    in  sys_clk : clock    @Sys
    in  pix_clk : clock    @Pix
    in  rst     : reset(sync, active_high)
    in  pixel   : bool     @Sys
    out lit     : bool     @Pix

    reg(sys_clk) wa : u6 = 0
    reg(pix_clk) ra : u6 = 0
    on sys_clk {
        wa <= wa + 1
    }
    on pix_clk {
        ra <= ra + 1
    }

    let fs = FrameStore {
        sys_clk: sys_clk,
        wr_addr: wa as bits<6>,
        wr_data: pixel,
        wr_en:   true,
        pix_clk: pix_clk,
        rd_addr: ra as bits<6>,
    }
    lit = fs.rd_data
}

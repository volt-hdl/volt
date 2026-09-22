//~ W3010
// ADR-0065 R5': the write clock of the dual-clock RAM also drives a
// register of its own (the write address counter), so the generated rst
// is sampled by both clocks again. The fix is the raw reset port.
domain Sys {
    clock = posedge
}

domain Pix {
    clock = posedge
}

module FrameStore {
    in  sys_clk : clock    @Sys
    //~^ ERROR W3010
    in  wr_data : bool     @Sys
    in  pix_clk : clock    @Pix
    in  rd_addr : bits<6>  @Pix
    out rd_data : bool     @Pix

    reg(sys_clk) wa : u6 = 0
    on sys_clk {
        wa <= wa + 1
    }

    let mem = AsyncDualPortRam<bool, 64> {
        wr_clk:  sys_clk,
        wr_addr: wa as bits<6>,
        wr_data: wr_data,
        wr_en:   true,
        rd_clk:  pix_clk,
        rd_addr: rd_addr,
    }
    rd_data = mem.rd_data
}

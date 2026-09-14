//~ E3014
// One symbolic domain, two clocks (ADR-0047): ExtRegFile declares both
// of its clock ports in @Core, i.e. the wrapped SV module is single
// domain. Binding the two ports to clocks from different domains
// would silently create a CDC path inside the black box.
domain SysDomain {
    clock = posedge
    reset = sync active_high
}

domain PixDomain {
    clock = posedge
    reset = sync active_high
}

extern module ExtRegFile {
    in  wr_clk  : clock @Core
    in  rd_clk  : clock @Core
    in  wr_addr : u4    @Core
    in  wr_data : u8    @Core
    in  wr_en   : bool  @Core
    in  rd_addr : u4    @Core
    out rd_data : u8    @Core
}

module Bad {
    in  sys_clk  : clock @SysDomain
    in  sys_addr : u4    @SysDomain
    in  sys_data : u8    @SysDomain
    in  sys_en   : bool  @SysDomain

    in  pix_clk  : clock @PixDomain
    in  pix_addr : u4    @PixDomain
    out pix_q    : u8    @PixDomain

    let rf = ExtRegFile {
        wr_clk:  sys_clk,
        rd_clk:  pix_clk,
        //~^ ERROR E3014
        wr_addr: sys_addr,
        wr_data: sys_data,
        wr_en:   sys_en,
        rd_addr: pix_addr,
    }
    pix_q = rf.rd_data
}

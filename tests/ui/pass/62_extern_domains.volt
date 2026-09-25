// Extern module with symbolic clock domains (ADR-0047). @Src and @Dst
// are declared nowhere: inside an `extern module` an unknown @Name is
// a symbolic domain parameter. Each instantiation binds it to a real
// domain through the clock connection (K8), and every other port of
// the extern is then checked against that binding -- an extern CDC
// primitive is no longer a blind spot for the compiler.
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

// Single-clock extern: no annotation needed, K2 applies at the boundary.
extern module ExtDelay {
    in  clk : clock
    in  d   : u8
    out q   : u8
}

module Bridge {
    in  sys_clk   : clock @SysDomain
    in  sys_d     : u8    @SysDomain
    in  sys_en    : bool  @SysDomain
    out sys_full  : bool  @SysDomain

    in  pix_clk   : clock @PixDomain
    in  pix_en    : bool  @PixDomain
    out pix_d     : u8    @PixDomain
    out pix_empty : bool  @PixDomain

    let f = ExtAsyncFifo {
        wr_clk:  sys_clk,
        wr_data: sys_d,
        wr_en:   sys_en,
        rd_clk:  pix_clk,
        rd_en:   pix_en,
    }
    sys_full  = f.wr_full
    pix_empty = f.rd_empty

    // Output ports of the extern carry the bound domain: f.rd_data is
    // @PixDomain here, so it may feed a PixDomain-clocked extern.
    let dly = ExtDelay {
        clk: pix_clk,
        d:   f.rd_data,
    }
    pix_d = dly.q
}

// Output net (ADR-0079):
//~ NET-SKIP: the extern modules have no @source body on purpose (the fixture shows domain binding); the design is incomplete without external SV

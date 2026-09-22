// ADR-0065 R5' fixed: a raw synchronous reset is released synchronously
// to each clock by its own chain, so W3010 does not apply.
domain Sys {
    clock = posedge
}

domain Pix {
    clock = posedge
}

module Video {
    in  sys_clk : clock @Sys
    in  pix_clk : clock @Pix
    in  rst     : reset(sync, active_high)
    in  a       : u8   @Sys
    out b       : u8   @Sys
    out c       : bool @Pix

    reg(sys_clk) ra : u8 = 0
    reg(pix_clk) rc : bool = false
    on sys_clk {
        ra <= a
    }
    on pix_clk {
        rc <= !rc
    }
    b = ra
    c = rc
}

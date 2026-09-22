//~ W3010
// Both domains use the default synchronous reset, so one generated rst
// port is sampled by two unrelated clocks (ADR-0065 R5').
domain Sys {
    clock = posedge
}

domain Pix {
    clock = posedge
}

module Video {
    in  sys_clk : clock @Sys
    //~^ ERROR W3010
    in  pix_clk : clock @Pix
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

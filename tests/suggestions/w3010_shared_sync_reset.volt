// suggestion: W3010
// Two domains share the generated synchronous reset port: the fix declares
// the raw reset port, and the compiler adds one reset synchronizer per clock.
domain Sys {
    clock = posedge
}

domain Pix {
    clock = posedge
}

module Video {
    in  sys_clk : clock @Sys
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

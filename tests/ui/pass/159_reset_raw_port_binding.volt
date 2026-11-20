// ADR-0124: a reset port bound to a reset input port of the parent (here
// in the shorthand form). The parent has no register on clk, so the child
// synchronizes the reset itself (no E3003).

module Child {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    reg r : u8 = 5
    on clk {
        r <= r + 1
    }
    v = r
}

module RawResetBinding {
    in  clk  : clock
    in  rst  : reset(sync, active_high)
    out v    : u8

    let c = Child { clk, rst }
    v = c.v
}

//~ E2003
// #340: a raw reset passed through sync() and bound to a child's reset
// port never reached the child. The two flip-flops of sync() are cleared
// by this module's own reset, whose synchronizer comes from the same port
// (sv-mapping.md §8: the stages are reset to 0; ADR-0065 §2), so for
// exactly as long as rst was asserted they held 0: the child's reset input
// stayed 0 in every cycle, and its register counted through every reset
// from its power-on value. `volt check` reported nothing. ADR-0065 §1
// allows only a reset input port of the parent as the value of a reset
// port binding, and ADR-0124 makes both halves E2003: a reset as the
// source of sync() (rule 1) and a reset port bound to anything but a
// reset input port (rule 2). The right form gives Child no reset port: its
// automatic port gets this module's synchronized reset. Fixed by the pull
// request of ADR-0124.

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

module Top {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    let rst_s = sync(rst, clk)
    let c = Child { clk: clk, rst: rst_s }
    v = c.v
}

// ADR-0124 (#340): the way to reset a submodule from a raw reset. The child
// declares no reset port; its automatic port is connected to the parent's
// synchronized reset for its clock (ADR-0065 §1–2).

module Child {
    in  clk : clock
    out v   : u8

    reg r : u8 = 5
    on clk {
        r <= r + 1
    }
    v = r
}

module AutomaticResetPort {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    let c = Child { clk: clk }
    v = c.v
}

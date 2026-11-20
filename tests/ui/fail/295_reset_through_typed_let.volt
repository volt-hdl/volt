//~ E2003
// ADR-0124, rule 2: the typed form of the same let. Both forms are one
// rule now; before, only the typed one was rejected (E0003, at build).

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

module ResetThroughTypedLet {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    let rst_s : reset(sync, active_high) = rst
    let c = Child { clk: clk, rst: rst_s }
    //~^ ERROR reset port 'rst' of instance 'c' is not bound to a reset input port
    v = c.v
}

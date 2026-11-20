//~ E2003
// #340: a raw reset passed through sync() never reaches the child. The
// synchronizer's flip-flops are cleared by this module's own synchronized
// reset, which comes from the same port, so they hold 0 for exactly as
// long as rst is asserted (ADR-0124, rule 1). The binding is the second
// E2003: the value is not a reset input port (rule 2).

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

module ResetThroughSync {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    let rst_s = sync(rst, clk)
    //~^ ERROR sync() cannot carry a reset
    let c = Child { clk: clk, rst: rst_s }
    v = c.v
}

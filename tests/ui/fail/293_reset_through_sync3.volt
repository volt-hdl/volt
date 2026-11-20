//~ E2003
// ADR-0124, rule 1: sync3() is the same data synchronizer with three
// stages; a reset through it is cleared while it is asserted.

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

module ResetThroughSync3 {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    let rst_s = sync3(rst, clk)
    //~^ ERROR sync3() cannot carry a reset
    let c = Child { clk: clk, rst: rst_s }
    v = c.v
}

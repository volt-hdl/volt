// suggestion: E2003
// #340 (ADR-0124): a raw reset given to a submodule through sync(). The
// fix removes the let, the binding and the child's reset port; the child's
// automatic reset port then gets this module's synchronized reset.
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
    out n   : u8

    reg n_r : u8 = 0
    on clk {
        n_r <= n_r + 1
    }
    n = n_r
    let rst_s = sync(rst, clk)
    let c = Child { clk: clk, rst: rst_s }
    v = c.v
}

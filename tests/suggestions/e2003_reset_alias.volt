// suggestion: E2003
// ADR-0124: a raw reset given to a submodule through a let, as the first
// binding. The fix removes the let, the binding with its comma and the
// child's reset port, which is the child's first port.
module Child {
    in  rst : reset(sync, active_high)
    in  clk : clock
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

    let rst_s = rst
    let c = Child { rst: rst_s, clk: clk }
    v = c.v
}

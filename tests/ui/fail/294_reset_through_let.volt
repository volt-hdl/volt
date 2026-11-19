//~ E2003
// ADR-0124, rule 2: a reset copied into an untyped let is an ordinary
// signal. Bound as 'rst: rst', the parent and the child would synchronize
// the same reset twice on clk (E3003); through the let the check did not
// see it.

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

module ResetThroughLet {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8
    out n   : u8

    reg n_r : u8 = 0
    on clk {
        n_r <= n_r + 1
    }
    n = n_r
    let rst_s = rst
    let c = Child { clk: clk, rst: rst_s }
    //~^ ERROR reset port 'rst' of instance 'c' is not bound to a reset input port
    v = c.v
}

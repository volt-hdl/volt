//~ E2003
// ADR-0124, rule 2: a multiplexer of resets is combinational logic on the
// reset path; it can glitch, and no synchronizer releases its output.

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

module ResetThroughIf {
    in  clk : clock
    in  rst : reset(sync, active_high)
    in  sel : bool
    out v   : u8

    let c = Child { clk: clk, rst: if sel { rst } else { rst } }
    //~^ ERROR reset port 'rst' of instance 'c' is not bound to a reset input port
    v = c.v
}

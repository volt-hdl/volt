//~ E3003
// #371: two instances synchronize the same raw reset on the same clock,
// and that clock is an output of an instance (a PLL), not a clock port of
// the parent. ADR-0065 R6: at most one chain per (clock, raw reset) pair
// in the instance tree. Before the fix the site was dropped because its
// clock is not one of Top's clock ports, and `volt check` was silent.
domain Core { clock = posedge, reset = async active_low }

extern module Pll {
    in  clk_in : clock @Core
    out clk_o  : clock @Core
}

module Child {
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q     : u8
    reg r : u8 = 0
    on clk { r <= d }
    q = r
}

module Top {
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q1    : u8
    out q2    : u8
    let p = Pll { clk_in: clk }
    let a = Child { clk: p.clk_o, rst_n: rst_n, d: d }
    let b = Child {
        clk: p.clk_o,
        rst_n: rst_n,
        //~^ ERROR E3003
        d: d,
    }
    q1 = a.q
    q2 = b.q
}

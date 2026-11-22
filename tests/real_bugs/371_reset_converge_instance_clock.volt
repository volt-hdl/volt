//~ E3003
// #371: two sibling instances synchronized the same raw reset separately
// on the same clock, and that clock is an output of an extern instance (a
// PLL). `volt check` was silent: R6 counted only a clock port of the
// parent, so a clock from an instance output was dropped. Yosys's
// flattened netlist had two independent `rst_n` synchronizer chains on
// one clock net, releasing `a.r` and `b.r` independently. ADR-0065 R6: at
// most one chain per (clock, raw reset) pair in the instance tree, E3003.
// Fixed by the pull request that closes #371.
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
    let b = Child { clk: p.clk_o, rst_n: rst_n, d: d }
    q1 = a.q
    q2 = b.q
}

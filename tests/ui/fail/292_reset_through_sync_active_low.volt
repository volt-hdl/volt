//~ E2003
// ADR-0124: an active-low reset through sync() is rejected too. Its
// circuit happens to work (the cleared value 0 is the asserted level), but
// the extern leaves reset two cycles after the registers beside it, and
// the rule does not depend on the polarity.

domain D {
    clock = posedge
    reset = async active_low
}

extern module Probe {
    in  clk   : clock @D
    in  rst_n : reset(async, active_low)
    out o     : bool @D
}

module ActiveLowThroughSync {
    in  clk   : clock @D
    in  rst_n : reset(async, active_low)
    out o     : bool @D

    reg seen : bool = false
    on clk {
        seen <= true
    }
    let rst_s = sync(rst_n, clk)
    //~^ ERROR sync() cannot carry a reset
    let p = Probe { clk: clk, rst_n: rst_s }
    o = p.o && seen
}

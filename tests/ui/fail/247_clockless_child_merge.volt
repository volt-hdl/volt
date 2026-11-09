//~ E3001
// ADR-0114 K3: a clockless module that joins inputs from two clock
// domains in one output is a gate between the domains. The error is at
// the instance, even when the output is not read (the gate is in the
// netlist), the same rule as a function call (K5).
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module And2 {
    in  a : bool
    in  b : bool
    out y : bool
    y = a && b
}

module Merge {
    in  fast_clk  : clock @Fast
    in  _slow_clk : clock @Slow
    in  fast_flag : bool  @Fast
    in  slow_in   : bool  @Slow
    out fast_out  : bool  @Fast

    let g = And2 { a: fast_flag, b: slow_in }
    //~^ ERROR instance 'g' combines two clock domains in 'And2.y'
    reg r : bool = false
    on fast_clk {
        r <= fast_flag
    }
    fast_out = r
}

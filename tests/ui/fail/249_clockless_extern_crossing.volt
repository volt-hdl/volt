//~ E3001
// ADR-0114 K4: the body of an extern without a clock is not visible, so
// each of its unannotated outputs depends on every input (worst case).
// i.y carries @Fast; assigning it to @Slow is a crossing.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

extern module Inv {
    in  a : bool
    out y : bool
}

module CdcBridge {
    in  _fast_clk : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow

    let i = Inv { a: fast_flag }
    slow_flag = i.y
    //~^ ERROR direct assignment between clock domains
}

// Output net (ADR-0079):
//~ NET-SKIP: the extern has no @source body on purpose (the fixture shows the domain rule); the design is incomplete without external SV

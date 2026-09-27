//~ E0003
// ADR-0090 sec. 3: sync() builds a register chain at module level; inside
// an expression it has no mapping — bind it first.

domain Fast {
    clock = posedge
    reset = sync active_high
}
domain Slow {
    clock = posedge
    reset = sync active_high
}

module Inverted {
    in  fclk : clock @Fast
    in  sclk : clock @Slow
    in  x    : bool @Fast
    out o    : bool @Slow

    reg q : bool = false
    on fclk { q <= x }
    o = !sync(x, sclk)
//~^ ERROR sync() inside an expression or a block
}

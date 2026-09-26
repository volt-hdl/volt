//~ E3001
// ADR-0083 Karar 8: a `comb` condition is an operand of every assignment
// it guards (domain-inference.md K5). `comb { if fs { y = sa } }` is the
// same hardware as `y = if fs { sa } else { .. }` and gets the same E3001.

domain Fast { clock = posedge, reset = sync active_high }
domain Slow { clock = posedge, reset = sync active_high }

module CombConditionCdc {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  fs       : bool  @Fast
    in  sa       : u8    @Slow
    out y        : u8    @Slow

    comb {
        if fs { y = sa } else { y = 0 }
        //~^ ERROR different clock domains cannot be combined combinationally
    }
}

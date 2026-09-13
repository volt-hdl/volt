//~ E3001
// prev(x) is evaluated in x's clock domain (ADR-0040). Comparing
// prev(fast_data) (@Fast) with slow_data (@Slow) inside one contract
// mixes two domains — the usual CDC violation E3001.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module Mixed {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_data : u8    @Fast
    in  slow_data : u8    @Slow
    out same      : bool  @Slow

    invariant: prev(fast_data) == slow_data
    //~^ ERROR E3001

    same = slow_data == 0
}

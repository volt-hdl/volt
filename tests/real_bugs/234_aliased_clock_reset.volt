//~ E3003
// #234: a clock port declared through a type alias was data for the reset
// domain checks, which classified a port by its written type. The two
// asynchronous domains below share the one generated reset port rst_n,
// whose release can be synchronous to at most one of the two clocks:
// written with `clock` the design is E3003, written with `Clk` `volt
// check` was silent, and `volt build --emit=sdc` wrote no constraint file
// either (a note and exit code 0). ADR-0070 §6: `type C = clock` works
// like `clock` in a port; ADR-0065 R5: a shared asynchronous reset port is
// E3003. Fixed by the pull request that closes #234.
type Clk = clock

domain Fast {
    clock = posedge
    reset = async active_low
}

domain Slow {
    clock = posedge
    reset = async active_low
}

module Bridge {
    in  fast_clk  : Clk @Fast
    in  slow_clk  : Clk @Slow
    in  fast_in   : bool  @Fast
    out fast_seen : bool  @Fast
    out slow_out  : bool  @Slow

    reg(fast_clk) seen : bool = false
    on fast_clk {
        seen <= fast_in
    }
    fast_seen = seen
    slow_out = sync(fast_in, slow_clk)
}

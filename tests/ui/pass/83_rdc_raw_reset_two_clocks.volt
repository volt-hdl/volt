// ADR-0065 sec. 2: the raw reset port feeds both asynchronous domains;
// the compiler adds a two-stage release synchronizer per clock
// (rst_sync_fast_clk_stage0/1, rst_sync_slow_clk_stage0/1).
domain Fast {
    clock = posedge
    reset = async active_low
}

domain Slow {
    clock = posedge
    reset = async active_low
}

module Bridge {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  rst_n     : reset(async, active_low)
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

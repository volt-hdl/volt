// Every field key a domain reads, each written once (#420): clock,
// frequency, reset and trust_level. Two domains may use the same key.

domain Fast {
    clock = posedge
    frequency = 100.mhz
    reset = sync active_high
    trust_level = public
}

domain Slow {
    clock = negedge, frequency = 25.mhz, reset = none, trust_level = public
}

module DomainEveryFieldOnce {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  rst      : reset(sync, active_high)
    in  d        : bool  @Fast
    out q        : bool  @Slow

    reg r : bool = false
    on fast_clk { r <= d }
    q = sync(r, slow_clk)
}

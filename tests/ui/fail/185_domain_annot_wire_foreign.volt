//~ E3001
// ADR-0088: the wire says @Slow, its driver is @Fast — a crossing without
// sync(). Before the annotation was dropped with W0020 and nothing was
// checked.

domain Fast {}
domain Slow {}

module WireForeign {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  a        : bool  @Fast
    out y        : bool  @Slow

    wire s : bool @Slow
    s = a
    //~^ ERROR direct assignment between clock domains
    y = s
}

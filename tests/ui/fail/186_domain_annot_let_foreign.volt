//~ E3001
// ADR-0088: a let annotated @Slow whose value is @Fast.

domain Fast {}
domain Slow {}

module LetForeign {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  a        : bool  @Fast
    out y        : bool  @Slow

    let s : bool @Slow = a
    //~^ ERROR direct assignment between clock domains
    y = s
}

//~ E3010
// Unannotated signal in a multi-clock design.
// domain-inference.md K3

domain Fast { clock = posedge, reset = sync active_high }
domain Slow { clock = posedge, reset = sync active_high }

module AmbiguousDomain {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  data     : u8
    //~^ ERROR cannot determine the signal's clock domain
    out result   : u8    @Fast

    result = data
}

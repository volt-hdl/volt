//~ E3011
// The same register is written from two different 'on' blocks.
// domain-inference.md K4

domain Fast { clock = posedge, reset = sync active_high }
domain Slow { clock = posedge, reset = sync active_high }

module RegisterTwoDomains {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  a : u8 @Fast
    in  b : u8 @Slow
    out r : u8 @Fast

    reg shared : u8 = 0
    //~^ ERROR register is written from more than one clock domain

    on fast_clk { shared <= a }
    on slow_clk { shared <= b }

    r = shared
}

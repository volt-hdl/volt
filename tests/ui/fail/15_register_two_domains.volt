//~ E3011
// Aynı register iki farklı 'on' bloğundan yazılıyor.
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
    //~^ ERROR register birden fazla saat alanından yazılıyor

    on fast_clk { shared <= a }
    on slow_clk { shared <= b }

    r = shared
}

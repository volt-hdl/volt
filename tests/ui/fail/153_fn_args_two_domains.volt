//~ E3001
// ADR-0081 Karar 9: fn domain-polimorfik; sonuç argümanların join'i —
// iki farklı saat alanından argüman çağrı yerinde E3001.
domain Fast { clock = posedge, reset = sync active_high }
domain Slow { clock = posedge, reset = sync active_high }

fn both(a: bool, b: bool) -> bool {
    a && b
}

module TwoDomains {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    in  slow_in   : bool  @Slow
    out y         : bool  @Slow

    y = both(fast_flag, slow_in)
//~^ ERROR E3001 different clock domains cannot be combined combinationally
}

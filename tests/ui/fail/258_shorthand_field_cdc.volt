//~ E3001
// #204: `Flag { fast_flag }` means `Flag { fast_flag: fast_flag }`. The
// literal keeps the domain of its field (domain-inference.md K5), so
// reading it in the Slow domain without sync() is a crossing (K6).
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

struct Flag { fast_flag : bool }

module Bridge {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_flag : bool  @Fast
    out slow_flag : bool  @Slow

    let w = Flag { fast_flag }
    slow_flag = w.fast_flag
    //~^ ERROR direct assignment between clock domains
}

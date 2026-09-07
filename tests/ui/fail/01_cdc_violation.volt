//~ E3001
// Direct connection between two different clock domains.
// Crossing is not allowed without a sync() bridge.

domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module CdcViolation {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  fast_data : u8    @Fast
    out slow_data : u8    @Slow

    slow_data = fast_data
    //~^ ERROR direct assignment between clock domains
}

// #332: a crossing between clock domains that exists only in a match arm
// that is never taken is not part of the design. The arm after the first
// wildcard and the arm an earlier arm covers are not written
// (sv-mapping.md §19.1, W2014), so the hardware has no path from
// `fast_data` to `slow_data`: no E3001, only the W2014 of each dead arm.

domain Fast {
    clock = posedge
    reset = none
}
domain Slow {
    clock = posedge
    reset = sync active_high
}

module DeadArmCrossing {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  s         : u2    @Slow
    in  fast_data : u8    @Fast
    out slow_data : u8    @Slow
    out slow_seq  : u8    @Slow
    out fast_out  : u8    @Fast

    reg r : u8 = 0
    reg f : u8 @Fast = 0

    comb {
        match s {
            _ => { slow_data = 1 }
            1 => { slow_data = fast_data }
        }
    }
    on slow_clk {
        match s {
            1 => { r <= 2 }
            1 => { r <= fast_data }
            _ => { r <= 3 }
        }
    }
    on fast_clk {
        f <= fast_data
    }
    slow_seq = r
    fast_out = f
}

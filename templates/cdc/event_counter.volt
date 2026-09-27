// Two clock domains and a safe crossing between them.
//
// Pulses arrive in the fast domain; the slow domain counts them. A
// signal may only cross between clock domains through an explicit
// bridge. Try `toggle_s = toggle_r` instead of the sync() below:
// the compiler rejects it with E3001 (`volt explain E3001`).

domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

pub module EventCounter {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  rst      : reset(sync, active_high)
    in  pulse    : bool  @Fast   // one-cycle pulse per event
    out count    : u8    @Slow

    // A one-cycle pulse is shorter than a slow clock period and could be
    // missed. Each pulse flips a level instead; the slow domain sees the
    // flip however long it takes. Events must be at least three slow
    // clock periods apart.
    reg toggle_r : bool = false
    on fast_clk {
        if pulse {
            toggle_r <= !toggle_r
        }
    }

    // sync() is a two-flop synchronizer for a single-bit signal. For
    // multi-bit data use the built-in AsyncFifo (`volt explain stdlib`).
    wire toggle_s : bool
    toggle_s = sync(toggle_r, slow_clk)

    reg seen_r  : bool = false
    reg count_r : u8   = 0
    on slow_clk {
        seen_r <= toggle_s
        if toggle_s != seen_r {
            count_r <= count_r + 1
        }
    }

    cover: count_r == 3

    count = count_r
}

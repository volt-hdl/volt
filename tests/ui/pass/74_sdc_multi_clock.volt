// SDC generation, two clock domains (ADR-0054, ADR-0065). Everything in
// build/constraints/Bridge.sdc comes from information the compiler
// already has: two create_clock lines from the domain frequencies and a
// constraint on the path into every generated synchronizer's first stage
// (the sync() below and the PulseSync). No set_clock_groups: any other
// path between the domains stays timed (--sdc-style=clock-groups gives
// the ADR-0054 output). A multicycle path on the accumulator is declared
// on the register itself: paths ending at `acc_r` get two cycles. The
// reset comes in raw, so each clock gets its own release chain and the
// file lists them under "Reset synchronizers" (ADR-0065).
domain Fast {
    clock = posedge,
    reset = sync active_high,
    frequency = 100.mhz,
}

domain Slow {
    clock = posedge,
    reset = sync active_high,
    frequency = 25_175.khz,
}

pub module Bridge {
    in  fast_clk : clock @Fast
    in  start    : bool  @Fast
    in  data     : u8    @Fast
    out acc      : u8    @Fast

    in  slow_clk : clock @Slow
    in  rst      : reset(sync, active_high)
    out started  : bool  @Slow
    out flag     : bool  @Slow

    reg go_r : bool = false
    @multicycle(2)
    reg acc_r : u8 = 0

    on fast_clk {
        go_r  <= start
        acc_r <= acc_r + data
    }
    acc = acc_r

    // 1-bit level: two-flop synchronizer.
    wire flag_s : bool
    flag_s = sync(go_r, slow_clk)
    flag = flag_s

    // Single-cycle pulse: toggle synchronizer.
    let ps = PulseSync {
        src_clk:  fast_clk,
        pulse_in: start,
        dst_clk:  slow_clk,
    }
    started = ps.pulse_out
}

// SDC generation, two clock domains (ADR-0054). Everything in
// build/constraints/Bridge.sdc comes from information the compiler
// already has: two create_clock lines from the domain frequencies, a
// `set_clock_groups -asynchronous` between the two domains, and a
// `set_false_path` into every generated synchronizer (the sync() below
// and the PulseSync). A multicycle path on the accumulator is declared
// on the register itself: paths ending at `acc_r` get two cycles.
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

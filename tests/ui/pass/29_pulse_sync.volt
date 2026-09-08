// Built-in PulseSync primitive (ADR-0027) -- carries a single-cycle
// pulse across clock domains with a toggle register plus edge detect.
//
// NOTE: this fixture intentionally triggers W3005. The toggle protocol
// drops pulses that arrive too close together, and the clock ratio is
// unknown at compile time, so the compiler reminds the designer on
// every PulseSync instantiation: keep at least 3 destination clock
// cycles between consecutive source pulses.
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module PulseBridge {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  p_in     : bool  @Fast
    out p_out    : bool  @Slow

    let ps = PulseSync {
        src_clk: fast_clk,
        pulse_in: p_in,
        dst_clk: slow_clk,
    }

    p_out = ps.pulse_out
}

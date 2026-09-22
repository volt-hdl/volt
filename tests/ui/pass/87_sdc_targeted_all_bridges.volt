// Targeted SDC (ADR-0065 sec. 4): every synchronizer kind the compiler
// generates, a raw reset port and a child instance in one design. The
// generated .sdc/.xdc constrain only the path into each synchronizer's
// first stage and the raw reset's assert path; no set_clock_groups, so
// any other path between the domains stays timed. The cell-name test
// (volt-driver sdc_emit_tests) checks every get_cells pattern against a
// register in the generated SV. Each domain takes a raw reset: one
// release synchronizer per clock (rst_sync_<clk>_stage0/1); the async
// domain uses it as an asynchronous clear, the sync domain as a
// synchronous reset.
domain Fast {
    clock = posedge,
    reset = async active_low,
    frequency = 100.mhz,
}

domain Slow {
    clock = posedge,
    reset = sync active_high,
    frequency = 25_175.khz,
}

module Relay {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  level    : bool  @Fast
    out seen     : bool  @Slow

    reg level_r : bool = false
    on fast_clk {
        level_r <= level
    }
    seen = sync(level_r, slow_clk)
}

module AllBridges {
    in  fast_clk  : clock @Fast
    in  slow_clk  : clock @Slow
    in  ext_rst_n : reset(async, active_low) @Fast
    in  ext_rst   : reset(sync, active_high) @Slow

    in  start   : bool @Fast
    in  din     : u8   @Fast
    in  push    : bool @Fast
    in  wr_addr : bits<4> @Fast
    in  pop     : bool @Slow
    in  rd_addr : bits<4> @Slow

    out go_s    : bool @Slow
    out start_s : bool @Slow
    out dout    : u8   @Slow
    out hs_out  : u8   @Slow
    out hs_ok   : bool @Slow
    out pulse   : bool @Slow
    out ram_out : u8   @Slow
    out relayed : bool @Slow

    reg go_r : bool = false
    on fast_clk {
        go_r <= start
    }

    // sync() from a register, sync3() from a port of the other domain
    // (capture register sync_start_src).
    go_s    = sync(go_r, slow_clk)
    start_s = sync3(start, slow_clk)

    let fifo = AsyncFifo<u8, 16> {
        wr_clk: fast_clk,
        wr_data: din,
        wr_en: push,
        rd_clk: slow_clk,
        rd_en: pop,
    }
    dout = fifo.rd_data

    let hs = HandshakeSync<u8> {
        src_clk: fast_clk,
        data_in: din,
        send: start,
        dst_clk: slow_clk,
    }
    hs_out = hs.data_out
    hs_ok  = hs.valid

    let ps = PulseSync {
        src_clk:  fast_clk,
        pulse_in: start,
        dst_clk:  slow_clk,
    }
    pulse = ps.pulse_out

    let ram = AsyncDualPortRam<u8, 16> {
        wr_clk:  fast_clk,
        wr_addr: wr_addr,
        wr_data: din,
        wr_en:   push,
        rd_clk:  slow_clk,
        rd_addr: rd_addr,
    }
    ram_out = ram.rd_data

    // The child keeps its automatic reset port and receives this
    // module's synchronized resets (ADR-0065 sec. 1).
    let u = Relay {
        fast_clk: fast_clk,
        slow_clk: slow_clk,
        level: start,
    }
    relayed = u.seen
}

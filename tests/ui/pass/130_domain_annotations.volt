// ADR-0088: a wire, a let (module or block) and a reg may carry a clock
// domain annotation like a port does. It is a CHECKED statement of
// intent: domain inference still runs, and a value from another domain
// is E3001. The annotation changes no generated SystemVerilog.

domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

module Annotated {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  rst      : reset(sync, active_high)
    in  req      : bool  @Fast
    out seen     : bool  @Slow
    out echo     : bool  @Fast

    // The crossing point is explicit: 'synced' lives in @Slow.
    wire synced : bool @Slow
    synced = sync(req, slow_clk)

    let held : bool @Fast = req
    reg last : bool @Fast = false
    reg count : u4 @Slow = 0

    on fast_clk {
        last <= held
    }

    on slow_clk {
        let bump : bool @Slow = synced
        if bump {
            count <= count + 1
        }
    }

    seen = synced && count != 0
    echo = last
}

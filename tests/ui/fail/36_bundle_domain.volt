//~ E3013
// A bundle is one interface: all of its fields must share a clock
// domain. Field-level annotations put 'data' in @Fast and 'ready' in
// @Slow, splitting the bundle across a CDC boundary (ADR-0039).
domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

struct port Bus {
    out data  : u8   @Fast
    in  ready : bool @Slow
}

module Split {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    out bus      : Bus
    //~^ ERROR E3013

    reg r : u8 = 0
    on fast_clk {
        r <= r + 1
    }
    bus.data = r
}

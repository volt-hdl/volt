// A button sampled in one clock domain drives a LED in another.

domain Fast {
    clock = posedge
    reset = sync active_high
}

domain Slow {
    clock = posedge
    reset = sync active_high
}

pub module Crossing {
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  rst      : reset(sync, active_high)
    in  button   : bool  @Fast
    out led      : bool  @Slow

    reg pressed_r : bool = false
    on fast_clk {
        pressed_r <= button
    }

    let pressed_r_sync = sync(pressed_r, slow_clk)

    reg led_r : bool = false
    on slow_clk {
        led_r <= pressed_r_sync
    }

    led = led_r
}

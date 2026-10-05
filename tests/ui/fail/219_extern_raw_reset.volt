//~ W3011
// The raw reset port `rst` feeds clk, so Volt puts a reset synchronizer
// (async assert, sync release) in front of count_r. The extern instance
// gets `rst` itself: its flip-flops leave reset on a different cycle than
// count_r, and nothing aligns their release to clk (issue #93).

extern module RisePulse {
    in  clk   : clock
    in  rst   : reset(sync, active_high)
    in  level : bool
    out rise  : bool
}

module ExternRawReset {
    in  clk    : clock
    in  rst    : reset(sync, active_high)
    in  button : bool
    out count  : u8

    let det = RisePulse { clk: clk, rst: rst, level: button }
    //~^ ERROR W3011

    reg count_r : u8 = 0
    on clk {
        if det.rise {
            count_r <= count_r + 1
        }
    }

    count = count_r
}

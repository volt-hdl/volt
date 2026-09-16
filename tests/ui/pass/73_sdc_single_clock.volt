// SDC generation, single clock (ADR-0054). The domain carries the
// frequency; `volt build --emit=sdc` writes build/constraints/Uart.sdc
// with `create_clock -name clk -period 20.000 [get_ports clk]`. The
// module states a requirement (at least 40 MHz for the 16x oversampler)
// and a path exception: the configuration register only changes while
// the receiver is idle, so its path to the sampler is a false path.
domain Sys {
    clock = posedge,
    reset = sync active_high,
    frequency = 50.mhz,
}

@timing(clk >= 40.mhz)
@false_path(from = cfg_r, to = sample)
pub module Uart {
    in  clk    : clock @Sys
    in  rx     : bool
    in  cfg    : u8
    in  cfg_we : bool
    out sample : bool
    out level  : u8

    reg cfg_r    : u8 = 0
    reg count_r  : u8 = 0
    reg sample_r : bool = false

    on clk {
        if cfg_we {
            cfg_r <= cfg
        }
        if count_r == cfg_r {
            count_r  <= 0
            sample_r <= rx
        } else {
            count_r <= count_r + 1
        }
    }

    sample = sample_r
    level  = cfg_r
}

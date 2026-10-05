// Blinky: the LED toggles every HALF_PERIOD clock cycles.

const HALF_PERIOD : u24 = 12_000_000

pub module Blinky {
    in  clk : clock
    out led : bool

    reg count_r : u24  = 0
    reg led_r   : bool = false

    on clk {
        if count_r == HALF_PERIOD - 1 {
            count_r <= 0
            led_r   <= !led_r
        } else {
            count_r <= count_r + 1
        }
    }

    led = led_r
}

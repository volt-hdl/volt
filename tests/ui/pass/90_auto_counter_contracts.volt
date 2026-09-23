// Automatic counter contracts (ADR-0066): every increment of `tick_r` is
// guarded by its wrap check, so `tick_r <= DIVISOR - 1` holds inductively
// (C2 invariant) and the wrap point is a cover target (C3). `free_r`
// increments without a guard (free-running, wraps at the type width):
// no contract. `@no_auto_contracts` opts `slow_r` out.

const DIVISOR : u8 = 6

module Divider {
    in  clk   : clock
    in  en    : bool
    out pulse : bool
    out free  : u8
    out slow  : u4

    reg tick_r  : u8 = 0
    reg free_r  : u8 = 0
    @no_auto_contracts
    reg slow_r  : u4 = 0

    on clk {
        if en {
            if tick_r == DIVISOR - 1 {
                tick_r <= 0
            } else {
                tick_r <= tick_r + 1
            }
        }
        free_r <= free_r + 1
        if slow_r >= 9 {
            slow_r <= 0
        } else {
            slow_r <= slow_r + 1
        }
    }

    pulse = en && tick_r == DIVISOR - 1
    free  = free_r
    slow  = slow_r
}

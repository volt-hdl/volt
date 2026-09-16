//~ E0017
// Unsupported timing constraint form (ADR-0054). Since @timing is
// enforced, a spelling the compiler cannot translate is an error, not a
// silent no-op: the delay below has no unit, so `set_max_delay` could
// not be written. The fix is `max_delay(a, y) <= 5.ns`.
domain Sys {
    clock = posedge,
    reset = sync active_high,
    frequency = 100.mhz,
}

@timing(max_delay(a, y) <= 5)
//~^ ERROR E0017: a delay needs a unit (ps, ns or us)
pub module Untimed {
    in  clk : clock @Sys
    in  a   : u8
    out y   : u8

    reg r : u8 = 0

    on clk {
        r <= a
    }

    y = r
}

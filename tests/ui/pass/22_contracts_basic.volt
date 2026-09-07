// Contract system (F4a): requires + ensures + invariant + cover.
// Type checking (E5004), scoping rules, and SVA generation build on
// this module: requires/ensures see the ports, invariant sees the
// registers, and cover sees both of them.

module SpiCtrl {
    in  clk   : clock
    in  start : bool
    in  speed : u8
    out busy  : bool
    out done  : bool

    requires:  speed <= 2
    ensures:   !start || busy
    invariant: !(busy_r && done_r)
    cover:     speed == 2 && done_r

    reg busy_r : bool = false
    reg done_r : bool = false

    on clk {
        busy_r <= start
        done_r <= busy_r
    }

    busy = busy_r
    done = done_r
}

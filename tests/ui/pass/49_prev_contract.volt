// prev() in contracts (ADR-0040): sequential properties that refer to
// the value of a signal one or more cycles ago. prev(x) is the previous
// cycle's value, prev(x, N) the value N cycles ago; both are legal only
// inside requires/ensures/invariant/cover/assert/assume and lower to
// $past(x) (SVA) or to helper registers (Yosys flow).
module Follower {
    in  clk   : clock
    in  start : bool
    out busy  : bool
    out seen  : bool

    // busy mirrors the previous value of start.
    invariant: busy == prev(start)
    // seen is a two-cycle delayed start.
    invariant: seen == prev(start, 2)
    // A handshake edge: start rose since the last cycle.
    cover: start && !prev(start)
    assume: !(prev(start) && prev(start, 2) && start)

    reg busy_r : bool = false
    reg seen_r : bool = false
    on clk {
        busy_r <= start
        seen_r <= busy_r
    }
    busy = busy_r
    seen = seen_r
}

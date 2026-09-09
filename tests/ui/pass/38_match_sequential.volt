// match in a sequential block (ADR-0032): lowers to case/default.
// Literal arms, an or-pattern arm and the mandatory `_` arm; the empty
// arm bodies of unreachable encodings keep the registers' values.

module MatchSequential {
    in  clk  : clock
    in  go   : bool
    out done : bool

    reg state_r : u2   = 0
    reg done_r  : bool = false

    on clk {
        match state_r {
            0 => {
                done_r <= false
                if go {
                    state_r <= 1
                }
            }
            1 | 2 => {
                state_r <= 3
            }
            _ => {
                done_r  <= true
                state_r <= 0
            }
        }
    }

    done = done_r
}

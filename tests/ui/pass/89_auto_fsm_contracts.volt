// Automatic FSM contracts (ADR-0066): a register written only with
// constants and matched in a sequential block is a state machine. The
// compiler adds one cover per transition (F3) and one per state that no
// transition cover reaches (F2); no invariant (F1: the mandatory `_` arm
// already defines every encoding). Generated RTL is unchanged.
//
//   0 -> 1 (go), 1 -> 2 (done), _ -> 0, and 3 on cancel (state cover)

module Sequencer {
    in  clk    : clock
    in  go     : bool
    in  done   : bool
    in  cancel : bool
    out busy   : bool

    reg state_r : u2 = 0

    on clk {
        match state_r {
            0 => {
                if go {
                    state_r <= 1
                }
            }
            1 => {
                if done {
                    state_r <= 2
                }
            }
            _ => {
                state_r <= 0
            }
        }
        if cancel {
            state_r <= 3
        }
    }

    busy = state_r != 0
}

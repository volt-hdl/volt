//~ E5001
// Unreachable FSM transition (ADR-0066): the machine never leaves state
// 0 (nothing drives it to 1), so the automatically generated transition
// cover `prev(state_r) == 1 && state_r == 2` can never be reached. The
// file COMPILES; the dead arm is found by
//     volt verify --mode cover 71_auto_fsm_unreachable_transition.volt
// (unreached cover -> E5001, exit code 6). Dead code the author did not
// know about is exactly what an automatic cover is for.

module DeadArm {
    in  clk  : clock
    in  go   : bool
    out busy : bool

    reg state_r : u2 = 0

    on clk {
        match state_r {
            0 => {
                if go {
                    state_r <= 0
                }
            }
            1 => {
                state_r <= 2
            }
            _ => {
                state_r <= 0
            }
        }
    }

    busy = state_r != 0
}

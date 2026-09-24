//~ E0014
// ADR-0074: a match on an enum without a '_' arm must name every
// variant. Here State::Stop has no arm — in a comb block that would be a
// latch; in a sequential block the missing action is a bug.
enum State { Idle, Run, Stop }

module Fsm {
    in  clk : clock
    in  go  : bool
    out busy : bool

    reg s : State = State::Idle

    on clk {
        match s {
//~^ ERROR does not cover every variant: missing State::Stop
            State::Idle => { if go { s <= State::Run } }
            State::Run => { s <= State::Stop }
        }
    }

    busy = s != State::Idle
}

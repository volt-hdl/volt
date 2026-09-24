//~ W2014
// ADR-0074: the second arm for State::Idle can never be taken.
enum State { Idle, Run }

module M {
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = State::Idle

    on clk {
        match s {
            State::Idle => { if go { s <= State::Run } }
            State::Idle => { s <= State::Idle }
//~^ ERROR unreachable arm
            State::Run => { s <= State::Idle }
        }
    }

    y = s == State::Run
}

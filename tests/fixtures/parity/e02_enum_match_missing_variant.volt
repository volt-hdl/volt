// parity: E0014
enum State { Idle, Run, Done }
module M {
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = State::Idle
    on clk {
        match s {
            State::Idle => { if go { s <= State::Run } }
            State::Run => { s <= State::Done }
        }
    }
    y = s == State::Done
}

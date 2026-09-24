// parity: E2003
enum State { Idle, Run, Done }
module M {
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = State::Idle
    on clk {
        match s {
            0 => { s <= State::Run }
            _ => { s <= State::Idle }
        }
    }
    y = s == State::Done
}

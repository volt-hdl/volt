// parity: E2003
enum State { Idle, Run, Done }
module M {
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = State::Idle
    on clk { if go { s <= State::Run } }
    y = s < State::Done
}

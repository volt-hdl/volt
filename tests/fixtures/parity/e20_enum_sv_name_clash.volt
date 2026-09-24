// parity: E1003
enum State { Idle, Run, Done }
module M {
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = State::Idle
    wire State_Run : bool
    State_Run = go
    on clk { if State_Run { s <= State::Run } }
    y = s == State::Done
}

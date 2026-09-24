// parity: E2003
enum State { Idle, Run, Done }
module M {
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = State::Idle
    on clk { s <= ~s }
    y = go && s == State::Done
}

// parity: E2009
enum State { Idle, Run, Done }
module M {
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = State::Idle
    in  raw : u2
    on clk { if go { s <= raw as State } }
    y = s == State::Done
}

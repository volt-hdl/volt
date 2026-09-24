// parity: ok
enum State { Idle, Run, Done }
module M {
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = State::Idle
    out c : u8
    on clk { if go { s <= State::Run } }
    c = s as u8
    y = (s as u2) == 1
}

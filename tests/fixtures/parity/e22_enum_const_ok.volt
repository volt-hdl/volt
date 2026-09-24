// parity: ok
enum State { Idle, Run, Done }
const START : State = State::Run
module M {
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = START
    on clk { if go { s <= START } else { s <= State::Done } }
    y = s == START
}

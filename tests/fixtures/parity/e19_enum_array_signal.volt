// parity: E0003
enum State { Idle, Run, Done }
module M {
    in  clk : clock
    in  go  : bool
    out y   : bool

    reg s : State = State::Idle
    reg arr : [State; 2] = [State::Idle; 2]
    on clk { if go { s <= State::Run  arr[0] <= State::Run } }
    y = s == State::Done && arr[0] == State::Run
}

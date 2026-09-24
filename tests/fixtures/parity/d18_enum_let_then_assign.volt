// parity: E4001
// drivers: 11 10
enum State { Idle, Run }
module M {
    in  clk : clock
    in  a : State
    in  b : State
    out y : bool

    let v : State = a
    v = b
    y = v == State::Run
}

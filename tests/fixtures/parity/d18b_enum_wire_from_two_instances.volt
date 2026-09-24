// parity: E4001
// drivers: 20 19
enum State { Idle, Run }
module Sub {
    in  x : bool
    out q : State

    q = if x { State::Run } else { State::Idle }
}

module M {
    in  a : bool
    in  b : bool
    out y : bool

    let s1 = Sub { x: a }
    let s2 = Sub { x: b }
    wire w : State
    w = s1.q
    w = s2.q
    y = w == State::Run
}

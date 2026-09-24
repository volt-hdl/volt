// parity: ok
enum St { Idle, Run }
struct P {
    a : u4
    b : bool
}
struct O {
    s : St
    p : P
}
module M {
    in  clk : clock
    in  x   : u4
    in  raw : u5
    in  q   : P
    out y   : bool
    out z   : u5
    out o   : O
    reg r : O = O { s: St::Idle, p: P { a: 0, b: false } }
    on clk {
        if x == 0 { r.p <= raw as P } else { r.s <= St::Run }
    }
    y = r.p == q && r.s != St::Idle
    z = q as u5
    o = r
}

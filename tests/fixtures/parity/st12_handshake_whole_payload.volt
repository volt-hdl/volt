// parity: E0003
struct P {
    a : u4
    b : bool
}
module M {
    in  clk : clock
    in  req : Handshake<P>
    out y   : bool
    reg p : P = P { a: 0, b: false }
    on clk { p <= req.data }
    req.ready = true
    y = p.b
}

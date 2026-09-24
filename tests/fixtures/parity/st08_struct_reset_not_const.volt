// parity: E2021
struct P {
    a : u4
    b : bool
}
module M {
    in  clk : clock
    in  x   : u4
    out y   : u4
    reg p : P = P { a: x, b: false }
    on clk { p.a <= x }
    y = p.a
}

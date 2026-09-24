// parity: E4012
struct P {
    a : u4
    b : bool
}
module M {
    in  x : u4
    out y : u4
    wire p : P
    p.a = x
    y = p.a
}

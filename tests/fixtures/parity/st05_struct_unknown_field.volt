// parity: E1008
struct P {
    a : u4
    b : bool
}
module M {
    in  p : P
    out y : u4
    y = p.nope
}

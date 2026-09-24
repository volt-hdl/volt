// parity: E2003
struct P {
    a : u4
    b : bool
}
module M {
    in  p : P
    in  q : P
    out y : bool
    y = p < q
}

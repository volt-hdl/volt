// parity: E1003
struct P {
    a : u4
    b : bool
}
module M {
    in  p   : P
    in  p_a : u4
    out y   : u4
    y = p.a ^ p_a
}

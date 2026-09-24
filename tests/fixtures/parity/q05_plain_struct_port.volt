// parity: E0003
struct P {
    a : u8
    b : bool
}

module M {
    in  p : P
    out y : u8

    y = p.a
}

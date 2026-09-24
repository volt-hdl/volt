// parity: ok
// drivers: ok
struct P {
    x : u8,
    y : u8,
}

module M {
    in  a : u8
    in  b : u8
    out o : u8

    wire s : P
    s.x = a
    s.y = b
    o = s.x + s.y
}

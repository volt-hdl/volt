// parity: E4001
// drivers: 15 14
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
    s.x = b
    s.y = a
    o = s.x + s.y
}

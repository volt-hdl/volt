// parity: E4001
// drivers: 18 17
module Sub {
    in  x : u8
    out q : u8

    q = x
}

module M {
    in  a : u8
    in  b : u8
    out y : u8

    let s = Sub { x: a }
    wire w : u8
    w = s.q
    w = b
    y = w
}

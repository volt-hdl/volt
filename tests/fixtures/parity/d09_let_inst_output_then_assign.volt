// parity: E4001
// drivers: 17 16
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
    let w = s.q
    w = b
    y = w
}

// parity: E4011
// drivers: ok
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
    s.q = b
    y = s.q
}

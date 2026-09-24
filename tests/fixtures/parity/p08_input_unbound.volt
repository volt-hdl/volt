// parity: E4011
module Child {
    in  a : u8
    in  b : u8
    out s : u8
    s = a
}
module M {
    in  x : u8
    out y : u8
    let c = Child { a: x }
    y = c.s
}

// parity: E2005
module Child {
    in  a : u8
    out s : u8
    s = a
}
module M {
    in  x : u8
    out y : u8
    wire w : u8
    let c = Child { a: x, s: w }
    y = c.s
}

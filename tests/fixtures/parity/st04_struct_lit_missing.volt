// parity: E2014
struct P {
    a : u4
    b : bool
}
module M {
    in  x : u4
    out y : u4
    let p : P = P { a: x }
    y = p.a
}

// parity: E0003
struct Pt { a : u8, b : u8 }
module M {
    in  x : u8
    out y : u8
    let p = Pt { a: x, b: x }
    y = p.a
}

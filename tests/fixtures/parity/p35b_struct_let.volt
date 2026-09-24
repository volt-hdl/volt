// parity: ok
struct Pt { a : u8, b : u8 }
module M {
    in  p : Pt
    out y : u8
    y = p.a
}

// parity: E2010
enum E : u3 { A = 0, B = 8 }
module M {
    in  a : u8
    out y : u8
    y = a
}

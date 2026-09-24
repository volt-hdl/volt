// parity: E2030
enum Op { A = 1, B = 1 }
module M {
    in  a : u8
    out y : u8
    y = a
}

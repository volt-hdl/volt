// parity: E2030
enum Op : u4 { Add = 0, Sub, Jal = 8 }
module M {
    in  a : u8
    out y : u8
    y = a
}

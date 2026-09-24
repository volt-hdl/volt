// parity: E2003
const C : [u8; 2] = [1, 2]
module M {
    in  x : u8
    out y : u8
    y = x + C
}

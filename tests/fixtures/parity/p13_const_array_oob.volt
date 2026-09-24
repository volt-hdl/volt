// parity: E2006
const C : [u8; 2] = [1, 2]
module M {
    out y : u8
    y = C[5]
}

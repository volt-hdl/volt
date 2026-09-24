// parity: E2005
const C : [u8; 2] = [1, 2u8 as u8]
module M {
    in  s : u1
    out y : u8
    y = C[s]
}

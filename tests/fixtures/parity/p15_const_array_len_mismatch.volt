// parity: E2003
const C : [u8; 3] = [1, 2]
module M {
    in  s : u2
    out y : u8
    y = C[s]
}

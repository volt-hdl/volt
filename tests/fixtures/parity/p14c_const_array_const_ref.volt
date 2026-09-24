// parity: ok
const K : u8 = 3
const C : [u8; 2] = [1, K + 1]
module M {
    in  s : u1
    out y : u8
    y = C[s]
}

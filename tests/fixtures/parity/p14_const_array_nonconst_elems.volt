// parity: E1001
const C : [u8; 2] = [1, 2 + foo]
module M {
    in  s : u1
    out y : u8
    y = C[s]
}

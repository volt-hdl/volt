// parity: E4001
// drivers: 9 8
module M {
    in  a : [u8; 2]
    in  b : u8
    out arr : [u8; 2]

    arr = a
    arr[1] = b
}

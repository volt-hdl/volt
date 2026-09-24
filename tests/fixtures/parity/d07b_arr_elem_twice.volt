// parity: E4001
// drivers: 9 8
module M {
    in  a : u8
    in  b : u8
    out arr : [u8; 2]

    arr[0] = a
    arr[0] = b
    arr[1] = b
}

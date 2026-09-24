// parity: E4001
// drivers: 10 8
module M {
    in  a : u8
    in  b : u8
    out arr : [u8; 4]

    arr[0] = a
    for i in 0..4 {
        arr[i] = b
    }
}

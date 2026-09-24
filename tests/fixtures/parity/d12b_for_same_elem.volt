// parity: E4001
// drivers: 8 8
module M {
    in  a : u8
    out arr : [u8; 2]

    for i in 0..2 {
        arr[0] = a
    }
    arr[1] = a
}

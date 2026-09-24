// parity: ok
// drivers: ok
module M {
    in  a : u8
    out arr : [u8; 2]

    for i in 0..2 {
        arr[i] = a
    }
}

// parity: ok
// drivers: ok
module M {
    in  a : u8
    in  b : u8
    out arr : [u8; 4]

    comb {
        for i in 0..2 {
            arr[i] = a
        }
    }
    arr[2] = b
    arr[3] = b
}

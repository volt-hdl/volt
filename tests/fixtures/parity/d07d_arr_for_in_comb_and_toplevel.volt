// parity: E4001
// drivers: 13 10
module M {
    in  a : u8
    in  b : u8
    out arr : [u8; 2]

    comb {
        for i in 0..2 {
            arr[i] = a
        }
    }
    arr[1] = b
}

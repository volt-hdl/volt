// parity: E4001
// drivers: 10 9
module M {
    in  a : [u8; 2]
    in  b : u8
    out arr : [u8; 2]

    for i in 0..2 {
        let t : u8 = a[i]
        t = b
        arr[i] = t
    }
}

// parity: ok
// drivers: ok
module M {
    in  a : u8
    in  b : u8
    out arr : [u8; 2]

    arr[0][3:0] = a[3:0]
    arr[0][7:4] = b[7:4]
    arr[1] = a
}

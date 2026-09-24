// parity: E4001
// drivers: 11 8
module M {
    in  a : u8
    in  b : u8
    out arr : [u8; 2]

    arr[0] = a
    arr[1][3:0] = b[3:0]
    arr[1][7:4] = a[7:4]
    arr[0][2] = true
}

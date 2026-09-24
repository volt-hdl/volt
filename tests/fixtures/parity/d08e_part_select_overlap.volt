// parity: E4001
// drivers: 10 9
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    y[0 +: 4] = a[3:0]
    y[5 -: 4] = b[3:0]
    y[7:6] = a[7:6]
}

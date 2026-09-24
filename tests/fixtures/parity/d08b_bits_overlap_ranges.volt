// parity: E4001
// drivers: 10 9
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    y[7:4] = a[7:4]
    y[5:0] = b[5:0]
}

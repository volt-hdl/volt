// parity: ok
// drivers: ok
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    y[7:4] = a[7:4]
    y[3:0] = b[3:0]
}

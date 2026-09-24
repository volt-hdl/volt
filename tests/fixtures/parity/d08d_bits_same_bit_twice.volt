// parity: E4001
// drivers: 10 9
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    y[0] = true
    y[0] = false
    y[7:1] = a[7:1]
}

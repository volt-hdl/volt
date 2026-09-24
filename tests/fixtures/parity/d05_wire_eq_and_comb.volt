// parity: E4001
// drivers: 11 10
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    wire w : u8
    w = a
    comb { w = b }
    y = w
}

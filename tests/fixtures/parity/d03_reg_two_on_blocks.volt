// parity: E4001
// drivers: 11 10
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    reg r : u8 = 0
    on clk { r <= a }
    on clk { r <= b }
    y = r
}

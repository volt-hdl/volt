// parity: E4001
// drivers: 10 10
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    for i in 0..2 {
        y = a
    }
}

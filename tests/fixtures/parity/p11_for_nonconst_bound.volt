// parity: E2005
module M {
    in  clk : clock
    in  n   : u8
    out y   : u8
    reg r : u8 = 0
    on clk {
        for i in 0..n { r <= r + 1 }
    }
    y = r
}

// parity: E2028
module M {
    in  clk : clock
    out y   : u8
    reg r : u8 = 0
    on clk {
        for i in 5..2 { r <= r + 1 }
    }
    y = r
}

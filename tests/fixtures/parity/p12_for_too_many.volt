// parity: E2005
module M {
    in  clk : clock
    out y   : u8
    reg r : u8 = 0
    on clk {
        for i in 0..5000 { r <= r + 1 }
    }
    y = r
}

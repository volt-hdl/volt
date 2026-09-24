// parity: E0003
module M {
    in  clk : clock
    in  x   : u8
    out y   : u8
    reg r : u8 = 0
    on clk {
        let t : u8 = x + 1
        r <= t
    }
    y = r
}

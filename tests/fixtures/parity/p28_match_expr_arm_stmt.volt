// parity: E0003
module M {
    in  clk : clock
    in  x   : u2
    out y   : u8
    reg r : u8 = 0
    on clk {
        match x {
            0 => r,
            _ => { r <= 2 }
        }
    }
    y = r
}

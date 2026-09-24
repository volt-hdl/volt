// parity: E0003
module M {
    in  clk : clock
    in  x   : u2
    in  g   : bool
    out y   : u8
    reg r : u8 = 0
    on clk {
        match x {
            0 if g => { r <= 1 }
            _ => { r <= 2 }
        }
    }
    y = r
}

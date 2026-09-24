// parity: E0014
module M {
    in  clk : clock
    in  x   : u2
    out y   : u8
    reg r : u8 = 0
    on clk {
        match x {
            v => { r <= 2 }
        }
    }
    y = r
}

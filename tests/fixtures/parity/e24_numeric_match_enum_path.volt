// parity: E0014 E2003
enum S { A, B }
module M {
    in  clk : clock
    in  x   : u2
    out y   : u8
    reg r : u8 = 0
    on clk {
        match x {
            S::A => { r <= 1 }
        }
    }
    y = r
}

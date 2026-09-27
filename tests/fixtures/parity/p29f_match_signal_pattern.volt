// parity: E1015
// A signal name in a pattern is rejected, never a binding (ADR-0085).
module M {
    in  clk : clock
    in  x   : u2
    in  lim : u2
    out y   : u8
    reg r : u8 = 0
    on clk {
        match x {
            lim => { r <= 2 }
            _ => { r <= 1 }
        }
    }
    y = r
}

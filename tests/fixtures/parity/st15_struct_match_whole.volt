// parity: E0003
struct P {
    a : u4
    b : bool
}
module M {
    in  clk : clock
    in  p   : P
    out y   : bool
    reg r : bool = false
    on clk {
        match p {
            _ => { r <= true }
        }
    }
    y = r
}

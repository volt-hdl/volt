// parity: E2005
const C : [u8; 2] = [1, 2]
module M {
    in  clk : clock
    out y   : u8
    reg r : [u8; 2] = C
    on clk { r <= C }
    y = r[0]
}

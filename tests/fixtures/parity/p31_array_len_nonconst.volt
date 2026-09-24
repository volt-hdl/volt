// parity: E2021
module M {
    in  clk : clock
    in  n   : u8
    in  x   : u8
    out y   : u8
    reg r : [u8; n] = [0; 4]
    on clk { r[0] <= x }
    y = r[0]
}

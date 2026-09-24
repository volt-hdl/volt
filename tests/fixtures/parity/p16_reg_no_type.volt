// parity: E2012
module M {
    in  clk : clock
    in  x   : u8
    out y   : u8
    reg r = 0u8
    on clk { r <= x }
    y = r
}

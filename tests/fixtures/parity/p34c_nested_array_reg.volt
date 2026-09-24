// parity: E0003
module M {
    in  clk : clock
    in  x   : u8
    out y   : u8
    reg r : [[u8; 2]; 2] = [[0; 2]; 2]
    y = x
}

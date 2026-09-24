// parity: E0003
domain D { clock = posedge
 reset = sync active_high }
module M {
    in  clk : clock @D
    out y   : u8
    reg r : u8 = 0
    on clk.reset { r <= 0 }
    y = r
}

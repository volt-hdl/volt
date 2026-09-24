// parity: E2005
domain D { clock = posedge
 reset = async active_low }
domain P { clock = posedge
 reset = sync active_high }
module Child {
    in  clk : clock @D
    in  d   : u8 @D
    out q   : u8 @D
    reg r : u8 = 0
    on clk { r <= d }
    q = r
}
module M {
    in  clk : clock @P
    in  x   : u8 @P
    out y   : u8 @P
    let c = Child { clk: clk, d: x }
    y = c.q
}

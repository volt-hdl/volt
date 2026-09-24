// parity: ok
domain D { clock = posedge
 reset = sync active_high }
module Child {
    in  clk : clock @D
    in  d   : u8
    out q   : u8
    reg r : u8 = 0
    on clk { r <= d }
    q = r
}
module M {
    in  clk : clock
    in  x   : u8
    out y   : u8
    let c = Child { clk: clk, d: x }
    y = c.q
}

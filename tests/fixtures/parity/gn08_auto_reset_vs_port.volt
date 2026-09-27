// parity: E1003
domain Fast {
    clock = posedge
    reset = sync active_low
}
module M {
    in  clk   : clock @Fast
    in  rst_n : bool @Fast
    out o     : bool @Fast
    reg r : bool = false
    on clk { r <= rst_n }
    o = r
}

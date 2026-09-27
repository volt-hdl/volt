// parity: E1003
domain Fast {
    clock = posedge
    reset = async active_low
}
module M {
    in  clk   : clock @Fast
    in  rst_n : reset(async, active_low)
    in  x     : bool @Fast
    out o     : bool @Fast
    reg r : bool = false
    on clk { r <= x }
    let rst_sync_clk_stage0 = r
    o = rst_sync_clk_stage0
}

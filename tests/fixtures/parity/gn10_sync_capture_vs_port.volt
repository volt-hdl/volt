// parity: E1003
domain Fast {
    clock = posedge
    reset = sync active_high
}
domain Slow {
    clock = posedge
    reset = sync active_high
}
module M {
    in  fclk       : clock @Fast
    in  sclk       : clock @Slow
    in  x          : bool @Fast
    in  sync_x_src : bool @Fast
    out o          : bool @Slow
    out p          : bool @Fast
    reg q : bool = false
    on fclk { q <= sync_x_src }
    p = q
    o = sync(x, sclk)
}

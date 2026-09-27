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
    in  fclk : clock @Fast
    in  sclk : clock @Slow
    in  x    : bool @Fast
    out o    : bool @Slow
    out p    : bool @Fast
    reg q : bool = false
    on fclk { q <= x }
    let sync_x_stage0 = !q
    p = sync_x_stage0
    o = sync(x, sclk)
}

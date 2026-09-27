// parity: ok
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
    reg q : bool = false
    on fclk { q <= x }
    let s : bool = sync3(q, sclk)
    o = !s
}

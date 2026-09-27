// parity: E2003
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
    in  x    : u8 @Fast
    out o    : bool @Slow
    reg q : u8 = 0
    on fclk { q <= x }
    let s = sync(x, sclk)
    o = s
}

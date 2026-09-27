//~ E1003
// ADR-0090: sync() builds 'sync_<source>_src' and 'sync_<source>_stage<i>'
// (named in the SDC/XDC bridge constraints).

domain Fast {
    clock = posedge
    reset = sync active_high
}
domain Slow {
    clock = posedge
    reset = sync active_high
}

module Bridge {
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
//~^ ERROR 'sync_x_stage0' is both let 'sync_x_stage0' and stage 0 of the sync() of 'x'
}

//~ E0003
// 'reset_cycles' is a reserved domain key: it was parsed and no pass read
// it, so the promised minimum reset length was never checked.

domain Sys {
    clock = posedge,
    reset = sync active_high,
    reset_cycles = 4
    //~^ ERROR not supported yet: domain key 'reset_cycles'
}

module DomainResetCycles {
    in  clk : clock @Sys
    out q   : u8

    reg r : u8 = 0
    on clk { r <= r + 1 }
    q = r
}

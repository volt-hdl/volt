//~ E1003
// #420: a domain with `reset = none` and then `reset = sync active_high`
// compiled without a diagnostic. The SV and the reset-domain checks took
// the last field and generated the reset synchronizer
// `rst_sync_clk_stage0/1`; the constraint model counted the domain as
// reset-free because one field said `none`, so the XDC had no
// `set_property ASYNC_REG TRUE` line for that chain (ADR-0054, the XDC
// row; ADR-0065 §2). The maintainer's decision in #420: a field key
// written twice in a domain declaration is E1003, which points at both
// fields (ADR-0140), and every pass reads the fields through one shared
// reader. Fixed by the pull request that closes #420.

domain Sys {
    clock = posedge
    frequency = 50.mhz
    reset = none
    reset = sync active_high
}

module M {
    in  clk : clock @Sys
    in  rst : reset(sync, active_high)
    in  d   : u8
    out q   : u8
    reg r : u8 = 0
    on clk { r <= d }
    q = r
}

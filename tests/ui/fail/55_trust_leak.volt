//~ E3009
// Information flow violation (ADR-0052): `key` is secret (@SecureCore),
// `debug_out` is public (@Debug). A direct assignment would leak key
// material to the debug side. The compiler names both trust levels and
// suggests declassify(expr, "reason") as the only sanctioned downgrade.
domain SecureCore {
    clock       = posedge
    reset       = sync active_high
    trust_level = secret
}

domain Debug {
    clock       = posedge
    reset       = sync active_high
    trust_level = public
}

module Leak {
    in  clk       : clock @SecureCore
    in  key       : u8    @SecureCore
    out key_out   : u8    @SecureCore
    out debug_out : u8    @Debug

    reg key_r : u8 = 0            // in the SecureCore clock: secret

    on clk {
        key_r <= key
    }

    key_out   = key_r
    debug_out = key_r
    //~^ ERROR secret data flows to a public output
}

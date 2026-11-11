//~ E3009
// #204: `Wrap { key }` means `Wrap { key: key }`. The literal carries the
// secret level of `key` (domain-inference.md K11, ADR-0052 §1-2), so a
// public output that reads it leaks the key.
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

struct Wrap { key : u8 }

module Leak {
    in  clk       : clock @SecureCore
    in  key       : u8    @SecureCore
    out debug_out : u8    @Debug

    let w = Wrap { key }
    debug_out = w.key
    //~^ ERROR secret data flows to a public output
}

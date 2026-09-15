// declassify (ADR-0052): the only sanctioned path from a higher trust
// level to a lower one. Each call needs a written reason and produces a
// W3008 warning -- the audit trail -- but no error. The same flows
// without `declassify` are E3009 (fail/55_trust_leak.volt).
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

module KeyPresence {
    in  clk       : clock @SecureCore
    in  load      : bool  @Debug
    in  key_in    : u8    @SecureCore
    out status    : bool  @Debug
    out busy      : bool  @Debug
    out key       : u8    @SecureCore

    // State encoding: 0 IDLE, 1 LOADED.
    reg state_r : u2 = 0
    reg key_r   : u8 = 0

    on clk {
        if load {
            key_r   <= key_in
            state_r <= 1
        }
    }

    key    = key_r
    status = declassify(key_r != 0, "key presence only")
    busy   = declassify(state_r != 0, "state visibility only")
}

//~ E0016
// declassify without a reason (ADR-0052): the justification string is
// mandatory -- it is the audit trail a security reviewer reads. A call
// without it is a syntax error, not a warning.
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

module NoReason {
    in  clk    : clock @SecureCore
    in  key    : u8    @SecureCore
    out status : bool  @Debug

    status = declassify(key != 0)
    //~^ ERROR declassify requires a reason
}

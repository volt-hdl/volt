//~ E3009
//~ W1001
// #211 (open): the trust annotation of a `let` is not read. `w` is
// declared public (`@Pub`, trust_level = public) and its value is a
// secret: domain-inference.md K1 (ADR-0088): "the annotation in a
// declaration fixes the signal's domain and is checked: a value in another
// domain is an error", and K11 (ADR-0052): a signal's level is its
// annotated domain's level, and a secret-to-public assignment is E3009.
// volt check gives no E3009: the trust check does not classify a `let`,
// so `w` takes the secret level of its value and `o = w` passes. The same
// design with `wire w : u8 @Pub` and `w = key` gets E3009 (metamorphic
// rewrite `let-to-wire`). W1001: the input port `clk` is not read
// (name-resolution.md, "unused input port"). Known failure until #211 is
// fixed.
domain Sec { clock = posedge, reset = sync active_high, trust_level = secret }
domain Pub { clock = posedge, reset = sync active_high, trust_level = public }

module M {
    in  clk : clock @Pub
    in  key : u8 @Sec
    out o   : u8 @Sec
    let w : u8 @Pub = key
    o = w
}

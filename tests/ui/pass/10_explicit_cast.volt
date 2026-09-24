// Explicit type cast — implicit widening is forbidden
// (ADR-0078: the inputs were 'small' and 'large' — both SystemVerilog
// keywords, so the generated SV did not compile; see ui/fail/122.)
module ExplicitCast {
    in  x8    : u8
    in  x16   : u16
    out sum   : u17
    out narrow: u8

    // Widening: explicit via as
    sum = (x8 as u16) + x16

    // Narrowing: explicit via as (produces a warning but is valid)
    narrow = x16 as u8
}

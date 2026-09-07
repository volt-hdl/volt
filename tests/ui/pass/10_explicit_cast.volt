// Explicit type cast — implicit widening is forbidden
module ExplicitCast {
    in  small : u8
    in  large : u16
    out sum   : u17
    out narrow: u8

    // Widening: explicit via as
    sum = (small as u16) + large

    // Narrowing: explicit via as (produces a warning but is valid)
    narrow = large as u8
}

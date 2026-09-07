//~ E2001
// Bit width mismatch: there is no implicit widening.
// An explicit cast (as) is required.

module WidthMismatch {
    in  small : u8
    in  large : u16
    out sum   : u16

    sum = small + large
    //~^ ERROR bit width mismatch: u8 and u16
}

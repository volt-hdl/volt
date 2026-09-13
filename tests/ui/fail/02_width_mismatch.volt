//~ E2001
// Bit width mismatch: the u16 operand does not fit the u8 target and
// operands never widen implicitly without a written target (ADR-0041).

module WidthMismatch {
    in  small : u8
    in  large : u16
    out sum   : u8

    sum = small + large
    //~^ ERROR bit width mismatch: u8 and u16
}

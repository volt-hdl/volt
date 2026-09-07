// Operator precedence: docs/spec/operator-precedence.md
module Precedence {
    in  a : u8
    in  b : u8
    in  c : u8
    out r1 : u16
    out r2 : bool
    out r3 : u8

    // a + (b * c)
    r1 = a as u16 + (b as u16) * (c as u16)

    // (a & 0x0F) == 0
    r2 = a & 0x0F == 0

    // (a << 2) | (b >> 1)
    r3 = (a << 2) | (b >> 1)
}

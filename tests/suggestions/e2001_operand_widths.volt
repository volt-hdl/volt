// suggestion: E2001
// Bitwise operands of different widths: the fix widens the narrow one.
module Mix {
    in  a : u8
    in  b : u16
    out y : u16

    y = a & b
}

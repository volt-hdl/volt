// suggestion: W0010
// '&' and '|' mixed without parentheses; the fix writes the current reading.
module Mix {
    in  a : u8
    in  b : u8
    in  c : u8
    out y : u8

    y = a & b | c
}

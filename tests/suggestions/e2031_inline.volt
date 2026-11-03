// suggestion: E2031
// A 3-bit value compared with a 4-bit sum: one fix wraps the sum at 3
// bits, the other widens the 3-bit side.
module CmpInline {
    in  a  : u3
    in  b  : u3
    out eq : bool

    eq = a == b + 1
}

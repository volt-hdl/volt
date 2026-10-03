// suggestion: E2004
// bits<N> is not a number and cannot be negated: the fix converts it to a
// signed number of the same width first.
module Neg {
    in  a : u8
    out y : i9

    y = -a[7:0]
}

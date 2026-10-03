// suggestion: E2003
// A bit selection is bits<N>; the fix converts it to a number.
module Nib {
    in  a : u8
    out y : u4

    y = a[3:0]
}

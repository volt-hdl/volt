// suggestion: E2003
// bits<4> into a u8 port: the fix converts to u4, which widens to u8.
module Nib {
    in  a : u8
    out y : u8

    y = a[7:4]
}

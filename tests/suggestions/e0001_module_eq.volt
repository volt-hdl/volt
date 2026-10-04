// suggestion: E0001
// A comparison on its own at module level is most likely a mistyped '='.
module Eq {
    in  a : u8
    out y : u8

    y == a
}

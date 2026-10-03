// suggestion: E0007
// A comb block assigns with '='; the fix replaces '<='.
module Pick {
    in  s : bool
    in  a : u8
    in  b : u8
    out y : u8

    comb {
        if s {
            y <= a
        } else {
            y = b
        }
    }
}

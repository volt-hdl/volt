// suggestion: E2028
// A module-level 'for' with its bounds reversed.
module F {
    in  a : u4
    out y : u4

    for i in 4..0 {
        y[i] = a[i]
    }
}

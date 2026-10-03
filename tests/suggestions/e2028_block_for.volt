// suggestion: E2028
// A 'for' inside a comb block with its bounds reversed.
module F {
    in  a : u4
    out y : u4

    comb {
        for i in 4..0 {
            y[i] = a[i]
        }
    }
}

// suggestion: E1008
// A misspelt field in a field access.
struct Pair {
    lo : u4,
    hi : u4,
}

module P {
    in  p : Pair
    out y : u4

    y = p.hj
}

// suggestion: E1008
// A misspelt field in a struct literal.
struct Pair {
    lo : u4,
    hi : u4,
}

module P {
    in  a : u4
    out y : Pair

    y = Pair { lo: a, hii: a }
}

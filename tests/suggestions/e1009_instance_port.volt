// suggestion: E1009
// A misspelt port name in an instance literal.
module Inc {
    in  x : u8
    out y : u8

    y = x + 1
}

module Top {
    in  a : u8
    out b : u8

    let i = Inc { xx: a }
    b = i.y
}

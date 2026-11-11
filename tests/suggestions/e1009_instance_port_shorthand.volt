// suggestion: E1009
// #204: a misspelt shorthand port is renamed and keeps its value:
// `Inc { xx }` becomes `Inc { x: xx }`.
module Inc {
    in  x : u8
    out y : u8

    y = x + 1
}

module Top {
    in  xx : u8
    out b  : u8

    let i = Inc { xx }
    b = i.y
}

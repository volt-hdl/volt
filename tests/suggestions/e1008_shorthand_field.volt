// suggestion: E1008
// #204: a misspelt shorthand field is renamed and keeps its value:
// `Pair { lo: a, hii }` becomes `Pair { lo: a, hi: hii }`.
struct Pair {
    lo : u4,
    hi : u4,
}

module P {
    in  a   : u4
    in  hii : u4
    out y   : Pair

    y = Pair { lo: a, hii }
}

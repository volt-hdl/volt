//~ E0010
// Comparison operators cannot be chained.
// Forbidden to prevent the mathematical fallacy.

module ComparisonChain {
    in  a : u8
    in  b : u8
    in  c : u8
    out r : bool

    r = a < b < c
    //~^ ERROR comparison operators cannot be chained
}

//~ E2011
// #144: literal arithmetic adapted to Trit must evaluate to -1, 0 or +1
// (type-inference.md §4, E2011); 1 + 1 = 2.

module LiteralTritOutOfRange {
    out t : Trit

    t = 1 + 1
    //~^ ERROR literal expression evaluates to 2, which does not fit in type Trit
}

//~ E2010
// #144: in a comparison the literal side takes the other side's type
// (type-inference.md §3.3, §4; ADR-0106 §1, §2). 3 - 1 = 2 does not fit
// u1; the comparison used to be accepted as x < 0.

module LiteralComparisonOverflow {
    in  x : u1
    out c : bool

    c = x < 3 - 1
    //~^ ERROR literal expression evaluates to 2, which does not fit in type u1
}

//~ E2010
// #144: 1 - 2 = -1 is outside the range of u2 (0..=3); it used to wrap
// silently to 3.

module LiteralNegativeInUnsigned {
    out y : u2

    y = 1 - 2
    //~^ ERROR literal expression evaluates to -1, which does not fit in type u2
}

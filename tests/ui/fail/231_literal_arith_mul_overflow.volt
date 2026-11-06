//~ E2010
// #144: 2 * 2 = 4 does not fit u2 (0..=3); it used to wrap silently to 0.

module LiteralMulOverflow {
    out y : u2

    y = 2 * 2
    //~^ ERROR literal expression evaluates to 4, which does not fit in type u2
}

//~ E2010
// #144: a typed let checks its literal arithmetic like an assignment:
// 9 / 2 = 4 does not fit u2.

module LiteralTypedLetOverflow {
    out y : u2

    let t : u2 = 9 / 2
    //~^ ERROR literal expression evaluates to 4, which does not fit in type u2
    y = t
}

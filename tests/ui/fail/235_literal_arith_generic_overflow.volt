//~ E2010
// #144: monomorphization replaces N with the literal 15 (ADR-0041), so
// N / 3 is literal arithmetic: 5 does not fit u2.

module Div3<const N: u32> {
    out y : u2

    y = N / 3
    //~^ ERROR literal expression evaluates to 5, which does not fit in type u2
}

module LiteralGenericOverflow {
    out y : u2

    let d = Div3<15> { }
    y = d.y
}

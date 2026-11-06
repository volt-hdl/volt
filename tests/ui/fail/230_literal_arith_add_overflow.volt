//~ E2010
// #144: 5 + 2 is literal arithmetic; its value 7 is computed exactly and
// must fit the target type u2 (type-inference.md §3.3, §4; const-eval.md
// §1, §4). It used to wrap silently to 3.

module LiteralAddOverflow {
    out y : u2

    y = 5 + 2
    //~^ ERROR literal expression evaluates to 7, which does not fit in type u2
}

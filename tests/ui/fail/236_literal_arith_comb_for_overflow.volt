//~ E2010
// #144: a comb-block for variable is a compile-time constant
// (const-eval.md §8); (i + 6) / 2 is 3, 3, 4, 4 for i = 0..4, and 4 does
// not fit u2.

module LiteralCombForOverflow {
    out ys : [u2; 4]

    comb {
        for i in 0..4 {
            ys[i] = (i + 6) / 2
            //~^ ERROR literal expression evaluates to 4, which does not fit in type u2
        }
    }
}

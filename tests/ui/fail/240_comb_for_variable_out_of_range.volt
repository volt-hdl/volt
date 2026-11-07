//~ E2010
// #144: a comb-block for variable is a compile-time constant
// (const-eval.md §8), adapted to the target type like an unsuffixed
// literal (type-inference.md §4): i = 2 does not fit u1. It used to be
// truncated silently.

module CombForVariableOutOfRange {
    out ys : [u1; 4]

    comb {
        for i in 0..4 {
            ys[i] = i
            //~^ ERROR literal expression evaluates to 2, which does not fit in type u1
        }
    }
}

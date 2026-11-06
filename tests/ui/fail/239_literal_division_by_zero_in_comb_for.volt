//~ E2023
// #144: a comb-block for variable is a compile-time constant
// (const-eval.md §8); 6 / i divides by zero for i = 0.

module LiteralDivisionByZeroInCombFor {
    out ys : [u4; 2]

    comb {
        for i in 0..2 {
            ys[i] = 6 / i
            //~^ ERROR division by zero
        }
    }
}

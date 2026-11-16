//~ E2010
// #152: the fit check of literal arithmetic with `comb` loop variables
// (added for #144) evaluated every combination of the loop variables but
// gave up silently above 65 536 combinations; the emitter still unrolled
// every combination and wrote the folded value cut to the context width.
// Here 41^3 = 68 921 combinations: the last value 40 * 40 * 40 = 64 000
// was written as 64 000 mod 32 768 = 31 232 without a diagnostic.
// type-inference.md §4 and const-eval.md §1, §8 (a loop variable is a
// constant; the exact value must fit the expected type): 64 000 does not
// fit u15 (0..=32767), E2010. The spec sets no combination limit. Found
// and fixed on the branch of PR #151 before it was merged.
module Cap {
    out y : u15
    comb {
        for i in 0..41 {
            for j in 0..41 {
                for k in 0..41 {
                    y = i * j * k
                }
            }
        }
    }
}

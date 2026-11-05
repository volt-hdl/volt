//~ W1002
// #128: the loop variable I shadows the const I (name-resolution.md §6:
// an inner scope may shadow, with W1002). The body of `pick` resolves I in
// the function's scope, whose parent is the root scope (ADR-0081 Karar
// 12.1): ys[k] = xs[1] for every k, not xs[k].

const I : u32 = 1

fn pick(x: [u8; 4]) -> u8 { x[I] }

module LoopVarShadowsInlinedConst {
    in  xs : [u8; 4]
    out ys : [u8; 4]

    comb {
        for I in 0..4 {
        //~^ ERROR 'I' shadows a definition in an outer scope
            ys[I] = pick(xs)
        }
    }
}

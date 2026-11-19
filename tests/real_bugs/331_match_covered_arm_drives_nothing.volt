//~ W2014
//~ E4002
// #331: `c` is assigned only in the second arm `0`, which the first arm
// `0` already covers. The first matching arm wins (ADR-0083 Karar 5,
// const-eval.md) and an arm that earlier arms cover is not emitted and
// gets W2014 (sv-mapping.md §19.1), so nothing drives `c`: E4002
// (type-inference.md §11.3). The driver analysis counted the covered
// arm's assignment, `volt check` reported only W2014 and the generated SV
// left the output undriven (Z). The same gap gave a false E4001 when `c`
// had a driver outside the block.
module M {
    in  s : u2
    out a : u8
    out c : u8
    comb {
        match s {
            0 => { a = 1 }
            0 => { a = 9; c = 3 }
            _ => { a = 7 }
        }
    }
}

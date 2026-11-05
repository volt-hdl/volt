//~ W1002
// #130: the module-level loop variable N shadows the generic parameter N
// (name-resolution.md §6). Inside the loop N is the loop variable:
// ys[k] = k, not ys[3] (ADR-0056 unrolls in the loop's scope).

module Shadow<const N: u32> {
    out ys : [u8; 2]

    for N in 0..2 {
    //~^ ERROR 'N' shadows a definition in an outer scope
        ys[N] = N
    }
}

module ModuleForShadowsGeneric {
    out ys : [u8; 2]
    let s = Shadow<3> { }
    ys = s.ys
}

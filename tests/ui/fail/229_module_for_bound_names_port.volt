//~ E2021
// #130: the bound of a module-level `for` is resolved in the loop's scope;
// the port K shadows the const K (name-resolution.md §2, §6), and a port
// is not a compile-time constant (const-eval.md §8).

const K : u32 = 2

module ModuleForBoundNamesPort {
    in  K : u8
    out y : [bool; 4]

    y[2] = false
    y[3] = false
    for i in 0..K {
    //~^ ERROR the bounds of 'for i' must be compile-time constants
        y[i] = true
    }
}

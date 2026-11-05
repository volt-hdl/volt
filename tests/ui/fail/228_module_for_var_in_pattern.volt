//~ E1015
// #130: a name in a pattern is a value and must be a constant
// (name-resolution.md §5.1, ADR-0085); a module-level loop variable is
// not one, as a loop variable in a block is not.

module ModuleForVarInPattern {
    in  s : u8
    out y : u8

    for i in 0..1 {
        y = match s {
            i => 1,
            //~^ ERROR 'i' in a pattern must be a constant, but it is a loop variable
            _ => 0,
        }
    }
}

//~ W1002
// #122, #130: a generic parameter that shadows a root item warns, as a
// port does (name-resolution.md §5, §6). In the body N is the parameter:
// y = 3 in Shadow_3.

const N : u32 = 7

module Shadow<const N: u32> {
//~^ ERROR 'N' shadows a definition in an outer scope
    out y : u8
    y = N
}

module GenericShadowsRootConst {
    out y : u8
    let s = Shadow<3> { }
    y = s.y
}

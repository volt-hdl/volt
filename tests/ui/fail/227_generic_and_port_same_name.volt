//~ E1003
// #130: generic parameters are the first declarations of the module scope
// (name-resolution.md §5); a port with the same name is a duplicate in the
// same scope (§6), also in a monomorphised module.

module Dup<const N: u32> {
    in  N : u8
    //~^ ERROR 'N' is already defined in this scope
    out y : u8
    y = N
}

module GenericAndPortSameName {
    in  s : u8
    out y : u8
    let d = Dup<3> { N: s }
    y = d.y
}

//~ E2001
// #204: the port-binding shorthand `Child { x }` means `Child { x: x }`.
// Binding a 16-bit signal to an 8-bit input port is a width error in
// both forms (type-inference.md: no implicit narrowing).
module Child {
    in  x : u8
    out y : u8
    y = x
}

module Top {
    in  x : u16
    out y : u8
    let c = Child { x }
    //~^ ERROR a 16-bit value does not fit in an 8-bit target
    y = c.y
}

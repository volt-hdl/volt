//~ E2010
// ADR-0074: an explicit value must fit the base type (u3: 0..7).
enum Op : u3 {
    A = 0,
    B = 8
//~^ ERROR does not fit in the 3-bit base type
}

module M {
    in  a : u8
    out y : u8

    y = a
}

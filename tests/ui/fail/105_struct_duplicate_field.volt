//~ E1003
// ADR-0077: every field name of a struct is unique.
struct Pixel {
    red   : u8
    green : u8
    red   : u8
//~^ ERROR declared twice
}

module M {
    in  a : u8
    out y : u8

    y = a
}

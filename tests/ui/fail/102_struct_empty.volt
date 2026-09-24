//~ E2013
// ADR-0077: a zero-width value is no signal — a plain struct needs at
// least one field.
struct Empty { }
//~^ ERROR has no fields

module M {
    in  a : u8
    out y : u8

    y = a
}

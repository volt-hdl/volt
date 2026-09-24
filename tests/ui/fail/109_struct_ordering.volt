//~ E2003
// ADR-0077 Karar 4: structs have no order; compare with == / !=, compare a
// field, or convert explicitly with 'as uN'.
struct Ver {
    major : u4
    minor : u4
}

module M {
    in  a : Ver
    in  b : Ver
    out y : bool

    y = a < b
//~^ ERROR cannot be ordered
}

//~ E4001
// ADR-0077 Karar 6 / ADR-0073: struct fields are partial targets; two
// sources for the same field are a double driver, named by the field.
struct Pair {
    lo : u4
    hi : u4
}

module M {
    in  a : u4
    in  b : u4
    out y : u8

    wire p : Pair
    p.lo = a
    p.hi = b
    p.lo = b
//~^ ERROR 'p.lo' is already driven
    y = p as u8
}

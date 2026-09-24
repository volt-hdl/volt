//~ E4012
// ADR-0077 Karar 6: a struct wire driven field by field must drive every
// field — the undriven one would be X/UNDRIVEN in SystemVerilog.
struct Pair {
    lo : u4
    hi : u4
}

module M {
    in  a : u4
    out y : u8

    wire p : Pair
//~^ ERROR struct field 'p.hi' is never driven
    p.lo = a
    y = p as u8
}

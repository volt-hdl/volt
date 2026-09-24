//~ E1003
// ADR-0077 Karar 5: struct fields become SV signals '<signal>_<field>'; a
// signal of the same name in the module would collide.
struct Pair {
    lo : u4
    hi : u4
}

module M {
    in  p    : Pair
//~^ ERROR 'p_lo' is already defined
    in  p_lo : u4
    out y    : u4

    y = p.hi ^ p_lo
}

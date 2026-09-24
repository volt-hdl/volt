//~ E2021
// ADR-0077 Karar 4: the reset value of a struct register is a constant
// struct value — a literal with constant fields or a const struct.
struct Pair {
    lo : u4
    hi : u4
}

module M {
    in  clk : clock
    in  a   : u4
    out y   : u4

    reg p : Pair = Pair { lo: a, hi: 0 }
//~^ ERROR not a compile-time constant
    on clk { p.lo <= a }
    y = p.lo ^ p.hi
}

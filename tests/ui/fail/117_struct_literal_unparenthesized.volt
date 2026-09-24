//~ E0001
// ADR-0077 Karar 6: in an 'if' or contract condition '{' starts the block,
// so a struct literal there must be parenthesized.
struct Pair {
    lo : u2
    hi : u2
}

module M {
    in  clk : clock
    in  lo  : u2
    out y   : u2

    reg p : Pair = Pair { lo: 0, hi: 0 }
    on clk { p.lo <= lo }

    cover: p == Pair { lo: 1, hi: 0 }
//~^ ERROR must be in parentheses
    y = p.lo
}

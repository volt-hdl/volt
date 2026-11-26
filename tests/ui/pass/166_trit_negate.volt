// #434: a Trit is negated with '-' (type-inference.md sec. 3.4: -Trit is a
// Trit; ADR-0142: '~' is not defined for a Trit). '-' works on a port, a
// register, a struct field, an element of a Trit array (each element on
// its own: '-' is not defined for the array itself) and inside a ternary
// MAC term (Trit * iN, sec. 3.3).

struct Weight {
    t  : Trit
    ok : bool
}

module TritNegate {
    in  clk  : clock
    in  w    : Trit
    in  x    : i8
    in  s    : Weight
    in  ws   : [Trit; 2]
    out neg  : Trit
    out term : i8
    out fld  : Trit
    out late : Trit
    out all  : [Trit; 2]

    reg r : Trit = 0
    on clk {
        r <= w
    }

    neg  = -w
    term = (-w) * x
    fld  = -s.t
    late = -r
    comb {
        for i in 0..2 {
            all[i] = -ws[i]
        }
    }
}

// #144: literal-only arithmetic is folded at compile time with its exact
// value (type-inference.md §3.3, §4; const-eval.md §1, §4) and the value
// fits each target: assignment, typed let, comparison, operand next to a
// signal, generic parameter and comb-block for variable.

module Div<const N: u32> {
    out q : u2

    q = N / 3
}

module LiteralArithFolded {
    in  x  : u2
    out a  : u2
    out b  : u2
    out c  : bool
    out d  : i4
    out e  : u2
    out g  : u2
    out ys : [u2; 2]

    a = 7 / 3
    let t : u2 = 100 * 3 / 150
    b = t
    c = x == 5 % 3
    d = -9 / 2
    e = x & (7 / 3)
    let dv = Div<7> { }
    g = dv.q
    comb {
        for i in 6..8 {
            ys[i - 6] = i / 3
        }
    }
}

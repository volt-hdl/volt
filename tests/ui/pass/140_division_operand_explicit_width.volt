// #150, #173: the width of a division written out explicitly. No W2015:
// the operands are cast to the width the division is meant to use, the
// division in a wider target is pushed to it (ADR-0041), and atom and
// literal operands are never cut.

const K : u8 = 3

module DivisionExplicitWidth {
    in  a : u8
    in  b : u8
    in  c : u8
    out q : u16
    out r : u9
    out s : u8
    out t : u9

    q = (a * b) as u16 / (c as u16)
    r = (a + b) / c
    s = a % K
    t = (a + b) / 3
}

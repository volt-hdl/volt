//~ W2015
// #150, #173: '/' does not widen (type-inference.md §3.3); the division
// is done at the width both operands share, 8 bits here, and the product
// (u8..u16, ADR-0025) is cut to it first. The rule changes in v0.2.0;
// until then the compiler warns. The output is generated as before.

module DivisionOperandCut {
    in  a : u8
    in  b : u8
    in  c : u8
    in  d : u8
    out o : u9

    o = a * b / c + d
//~^ ERROR this operand of '/' is cut to 8 bits before the division
}

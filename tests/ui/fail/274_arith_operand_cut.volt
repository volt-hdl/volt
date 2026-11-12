//~ W2016
// #266: '+', '-' and '*' are done at the width both operands share plus
// their own carry (type-inference.md §3.3). The product (i32..i64,
// ADR-0025) is cut to 33 bits before the subtraction, and the comparison
// reads the whole 33-bit result. The rule changes in v0.2.0; until then
// the compiler warns. The output is generated as before.

module ArithOperandCut {
    in  a : i32
    in  b : i32
    out p : bool

    p = a * b - a == -1048576
//~^ ERROR this operand of '-' is cut to 33 bits before the operation
}

// #266: the computation of ui/fail 274 written so that its value does not
// depend on the cut. No W2016: the intermediate result is in a 'let' of
// explicit width, and a result assigned to a declared width keeps only
// that many bits.

module ArithOperandExplicitWidth {
    in  clk  : clock
    in  a    : i32
    in  b    : i32
    in  x    : i16
    in  y    : i16
    out p    : bool
    out low : i33
    out sum  : i32

    let d : i64 = a * b - a
    p = d == -1048576
    low = a * b - a
    reg acc : i32 = 0
    on clk {
        acc <= acc + x * y
    }
    sum = acc
}

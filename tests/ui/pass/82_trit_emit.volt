// Trit SV mapping (ADR-0003): 2-bit two's-complement storage,
// +1 = 2'sb01, 0 = 2'sb00, -1 = 2'sb11. Trit * iN lowers to a select
// between x, -x and 0 -- the generated SV holds no multiplier.
// type-inference.md sec. 3.3 (Trit * Trit -> Trit, Trit +- Trit -> i3).
module TritDot {
    in  clk  : clock
    in  w    : [Trit; 4]
    in  x    : [i8; 4]
    in  sign : Trit
    out dot  : i16
    out s    : i3

    reg acc : i16 = 0

    // Each term is a select, widened to the i16 target (ADR-0041).
    let p0 : i16 = w[0] * x[0]
    let p1 : i16 = w[1] * x[1]
    let p2 : i16 = w[2] * x[2]
    let p3 : i16 = w[3] * x[3]
    let flip : Trit = sign * w[0]

    on clk {
        acc <= p0 + p1 + p2 + p3
    }

    dot = acc
    s   = flip + sign
}

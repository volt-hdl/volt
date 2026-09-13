// Const array declaration and access (ADR-0041, const-eval.md sec. 2).
// A constant index folds at compile time; a signal index is emitted as
// a localparam table in the generated SV.

const COEFFS : [i16; 4] = [1, -2, 3, 4]

module ConstArray {
    in  clk : clock
    in  x   : i16
    in  sel : u2
    out y   : i32
    out c   : i16

    reg taps : [i16; 4] = [0; 4]

    on clk {
        for i in 1..4 { taps[i] <= taps[i - 1] }
        taps[0] <= x
    }

    // Constant indices: COEFFS[i] is folded per unrolled iteration.
    wire acc : i32
    comb {
        acc = 0
        for i in 0..4 { acc = acc + taps[i] * COEFFS[i] }
    }

    y = acc
    // Signal index: table lookup.
    c = COEFFS[sel]
}

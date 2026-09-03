// Ternary MAC deseni — Trit × INT8
// type-inference.md §3.3
module TernaryMac {
    in  clk    : clock
    in  weight : Trit
    in  act    : i8
    out acc    : i16

    reg accum : i16 = 0

    // Trit * i8 → i8 (ternary MAC kuralı)
    let product = weight * act

    on clk {
        accum <= accum + (product as i16)
    }

    acc = accum
}

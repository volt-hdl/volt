//~ E2022
// #144: literal arithmetic is computed exactly in the 128-bit
// compile-time range (const-eval.md §1); a product outside it is a
// compile-time overflow (const-eval.md §4 `checked_mul`), not a wrap.

module LiteralCompileTimeOverflow {
    out y : u64

    y = 18446744073709551615 * 18446744073709551615
    //~^ ERROR compile-time overflow in a literal expression
}

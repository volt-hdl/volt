//~ E2010
// #272: literals in a context of 127 bits or more skipped the range check.
// `a * b` with u64 operands is u128 (type-inference.md §3.3, overflow
// widening); the literal takes the other side's type (type-inference.md
// §4, ADR-0106). -1 does not fit in u128, but it was accepted and written
// -128'd1, which is 2^128 - 1: `q` compared the product with 2^128 - 1.
// Right answer: the range check holds at every width (type-inference.md
// §4, literal_fits: a negative literal never fits uN), so E2010. Fixed
// by #265.
module M {
    in  a : u64
    in  b : u64
    out q : bool
    q = a * b == -1
}

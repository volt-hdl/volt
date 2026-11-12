//~ E2010
// #272: a * b is u128 (u64 operands); -1 does not fit in it. It was
// accepted and written -128'd1, which is 2^128 - 1.
module M {
    in  a : u64
    in  b : u64
    out q : bool
    q = a * b == -1
}

//~ E2022
// #207: the compile-time integer is an i128 (const-eval.md section 1);
// a literal outside its range is an overflow (section 4), not a wrapped
// value. 2^128 - 1 used to be read as -1, so K was 0 at compile time.
const K : u64 = 0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF / 0x1_0000_0000_0000_0001
module M {
    out k : u64
    k = K
}

//~ E2010
// #262: a const generic argument was not checked against the type of its
// parameter. K is declared u64, the argument is 2^80 - 1; it was accepted
// and the generated SV wrote the 80-bit value into `o`, a value that no
// u64 can hold. Right answer: a generic const argument must fit the type
// of its parameter, otherwise E2010 (const-eval.md §2, "Generic const
// parametreler"; the same rule as `let x : u64 = 0xFFFF_FFFF_FFFF_FFFF_FFFF`,
// type-inference.md §4). Fixed by #265.
module G<const K: u64> {
    out o : bits<128>
    o = K as bits<128>
}

module Top {
    out o : bits<128>
    let g = G<0xFFFF_FFFF_FFFF_FFFF_FFFF> { }
    o = g.o
}

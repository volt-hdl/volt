//~ E2010
// #262: the argument 2^80 - 1 does not fit in the parameter K : u64. It
// was accepted and the hardware saw an 80-bit value.
module G<const K: u64> {
    out o : bits<128>
    o = K as bits<128>
}
module Top {
    out o : bits<128>
    let g = G<0xFFFF_FFFF_FFFF_FFFF_FFFF> { }
    o = g.o
}

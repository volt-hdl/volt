//~ W2010
// #207, ADR-0115: an untyped literal that does not fit the target of its
// cast is cut to the target's low bits, like a narrowing cast, and the cut
// is reported (never silent). 0x1FF does not fit in 8 bits: 8'hFF.
module M {
    out o : bits<8>
    o = 0x1FF as bits<8>
}

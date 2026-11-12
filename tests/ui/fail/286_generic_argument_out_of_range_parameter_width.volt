//~ E2010
// #262: the width of K is the earlier parameter W. With W = 4, K = 16 does
// not fit in uint<4>; it was accepted and K as bits<8> wrote 16.
module G<const W: u32, const K: uint<W>> {
    out o : bits<8>
    o = K as bits<8>
}
module Top {
    out o : bits<8>
    let g = G<4, 16> { }
    o = g.o
}

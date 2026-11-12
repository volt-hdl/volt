//~ E0005
// #207: a literal holds at most 128 bits (IntLit { value: u128 },
// ast-nodes.md), also in a wider context. 2^128 does not fit: an
// explicit error, never a silently cut value.
module M {
    out o : bits<256>
    o = 0x1_0000_0000_0000_0000_0000_0000_0000_0000 as bits<256>
}

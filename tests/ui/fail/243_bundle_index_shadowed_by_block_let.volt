//~ E2008
// #136: a bundle array index is evaluated in the scope where it is used
// (name-resolution.md §2, §6). The block let K shadows the root const K,
// so rx[K] is indexed by a signal, not a constant (ADR-0056). It used to
// read the root const and select rx[0] silently.

const K : u32 = 0

module BundleIndexShadowedByBlockLet {
    in  rx : [Handshake<u8>; 2]
    out y  : u8

    rx[0].ready = true
    rx[1].ready = true

    comb {
        let K : u32 = 1
        y = rx[K].data
    }
}

//~ E2008
// #136: the parser evaluated the index of a bundle array through the root
// consts only, so `rx[K]` took the root `const K` (0) although the module
// `let K` shadows it: the design compiled silently to `y = rx_0_data`.
// The module `let K` is declared in the module scope and shadows the root
// const (name-resolution.md §2, §6); `rx[K]` therefore indexes with a
// signal, and the index into a bundle array must be a compile-time
// constant (ADR-0056 §3: E2008), as with a fresh name. E2008 is raised
// while the parser flattens bundles (ADR-0056), and a parse-stage error
// stops the semantic stages (ADR-0070 gated pipeline, ADR-0068), so the
// shadowing W1002 of name resolution is not reported. Fixed by #157 (the
// bundle step resolves through the scope chain, ADR-0081 appendix).
const K : u32 = 0

module M {
    in  rx : [Handshake<u8>; 2]
    out y  : u8
    let K : u32 = 1
    rx[0].ready = true
    rx[1].ready = true
    y = rx[K].data
}

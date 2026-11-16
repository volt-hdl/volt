//~ E1001
// #139: the parser's const evaluation collected the root consts of every
// file of the unit without file visibility, so the bundle array index
// `rx[K]` took the private `const K` (1) of other.volt and compiled silently
// to `y = rx_1_data`. A name of another file is visible only when it is
// `pub` and imported with `use` (ADR-0042 §3, name-resolution.md §3.3);
// `K` is neither, so it is undefined here: E1001, as `z = K` in a value
// position gives. Fixed by #164 (one visibility rule for the parser's
// unit-end steps).
use other::X;

module M {
    in  rx : [Handshake<u8>; 2]
    out y  : u8
    out z  : u32
    rx[0].ready = true
    rx[1].ready = true
    y = rx[K].data
    z = X
}

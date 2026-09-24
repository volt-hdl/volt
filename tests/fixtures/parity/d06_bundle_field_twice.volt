// parity: E4001
// drivers: 14 13
struct port Hs {
    out data : u8
    out valid : bool
}

module M {
    in  a : u8
    in  b : u8
    out hs : Hs

    hs.data = a
    hs.data = b
    hs.valid = true
}

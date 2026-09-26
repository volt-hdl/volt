// parity: E1013
// ADR-0083 §6 sondası.
module M {
    in  a : u8
    out y : u8
    comb {
        let logic = a
        y = logic
    }
}

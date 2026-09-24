// parity: E0003
// drivers: ok
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    comb {
        let t = a
        t = b
        y = t
    }
}

// parity: ok
// drivers: ok
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    in  c : bool
    out y : u8

    comb {
        y = a
        if c { y = b }
    }
}

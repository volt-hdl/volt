// parity: ok
// drivers: ok
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    let v = b
    y = a
}

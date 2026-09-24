// parity: E4001
// drivers: 10 9
module M {
    in  clk : clock
    in  a : u8
    in  b : u8
    out y : u8

    let v = a
    on clk { v <= b }
    y = v
}

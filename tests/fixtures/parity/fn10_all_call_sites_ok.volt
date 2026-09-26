// parity: ok
fn inc(a: u8) -> u8 {
    let t = a + 1
    t
}
module M {
    in  clk : clock
    in  a   : u8
    out y   : u8
    out z   : u8
    reg r : u8 = 0
    invariant: inc(r) != r
    on clk { r <= inc(r) }
    y = inc(a) + r
    comb { z = inc(a ^ r) }
}

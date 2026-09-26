// parity: E4001
// ADR-0083 §6 sondası.
module M {
    in  clk : clock
    in  a : u8
    out q : u8
    reg r : u8 = 0
    on clk {
        let t = a
        t <= a
        r <= t
    }
    q = r
}

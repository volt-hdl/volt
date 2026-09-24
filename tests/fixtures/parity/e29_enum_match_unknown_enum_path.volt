// parity: E1001
// ADR-0075: a pattern path that resolves to nothing (unknown enum 'Q')
// leaves coverage unknown — the gated check stays silent (no E0014
// cascade on top of the resolve error).
enum S { A, B, C }
module M {
    in  clk : clock
    out y   : u8
    reg s : S = S::A
    reg r : u8 = 0
    on clk {
        match s {
            S::A => { r <= 1 }
            Q::B => { r <= 2 }
        }
    }
    y = r
}

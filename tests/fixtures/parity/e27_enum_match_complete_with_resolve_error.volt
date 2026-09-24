// parity: E1001
// ADR-0075: the gated exhaustiveness check is silent for a match that
// names every variant.
enum S { A, B, C }
module M {
    in  clk : clock
    out y   : u8
    reg s : S = S::A
    reg r : u8 = 0
    on clk {
        match s {
            S::A => { r <= 1 }
            S::B => { r <= 2 }
            S::C => { if r == 0 { r <= undefined_name } }
        }
    }
    y = r
}

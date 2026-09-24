// parity: E0014 E1001
// ADR-0075: a resolve error elsewhere closes the type-check gate; the
// deferred enum exhaustiveness error must still be reported.
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
        }
        s <= undefined_name
    }
    y = r
}

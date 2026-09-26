// parity: ok
// ADR-0083 §6 sondası.
enum S { Idle, Run }
module M {
    in  clk : clock
    in  s   : S
    in  a   : u8
    out q   : u8
    reg r : u8 = 0
    on clk {
        let t = match s { S::Idle => a, S::Run => a + 1 }
        r <= (match s { S::Idle => t, _ => t ^ a })
    }
    q = r
}

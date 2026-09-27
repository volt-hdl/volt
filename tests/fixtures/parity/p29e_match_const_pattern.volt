// parity: ok
// A const name in a pattern is a value, not a binding (ADR-0085).
const LIMIT : u2 = 2
module M {
    in  clk : clock
    in  x   : u2
    out y   : u8
    reg r : u8 = 0
    on clk {
        match x {
            LIMIT => { r <= 2 }
            _ => { r <= 1 }
        }
    }
    y = r
}

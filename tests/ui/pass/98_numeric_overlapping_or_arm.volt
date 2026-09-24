// ADR-0075: an arm is unreachable only when EVERY value it names was
// already taken; '0 | 1' after '1' still selects 0, so it is kept (no
// W2014) and emitted as a case label.
module M {
    in  clk : clock
    in  x   : u2
    out y   : u8

    reg r : u8 = 0

    on clk {
        match x {
            1 => { r <= 1 }
            0 | 1 => { r <= 2 }
            _ => { r <= 0 }
        }
    }

    y = r
}

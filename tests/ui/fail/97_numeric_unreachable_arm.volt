//~ W2014
// ADR-0075: the second arm repeats value 1 (written as 0x1) — it can never
// be taken, exactly like a repeated enum variant (ADR-0074).
module M {
    in  clk : clock
    in  x   : u2
    out y   : u8

    reg r : u8 = 0

    on clk {
        match x {
            1 => { r <= 1 }
            0x1 => { r <= 2 }
//~^ ERROR unreachable arm
            _ => { r <= 0 }
        }
    }

    y = r
}

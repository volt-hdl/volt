//~ E2003
// ADR-0065 R3 (#356): a reset cannot be the scrutinee of a match, even
// without sync().

module ResetMatchScrutinee {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    out q   : u8

    reg r : u8 = 0
    on clk {
        r <= r + 1
    }
    q = r
    y = match rst { true => true, _ => false }
    //~^ ERROR a reset cannot be tested by match
}

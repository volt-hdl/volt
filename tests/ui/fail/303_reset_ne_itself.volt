//~ E2003
// ADR-0065 R3 (#356): a reset cannot be an operand of !=, even without
// sync().

module ResetNeItself {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out y   : bool
    out q   : u8

    reg r : u8 = 0
    on clk {
        r <= r + 1
    }
    q = r
    y = rst != rst
    //~^ ERROR a reset cannot be compared
}

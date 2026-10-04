// suggestion: E0007
// '<=' on a register at module level: the fix moves it into an 'on' block.
module R {
    in  clk : clock
    in  a   : u8
    out q   : u8

    reg r : u8 = 0
    r <= a
    q = r
}

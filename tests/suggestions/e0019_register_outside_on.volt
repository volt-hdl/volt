// suggestion: E0019
// A register assigned with '=' at module level: the fix moves the update
// into an 'on' block of the module's clock.
module R {
    in  clk : clock
    in  a   : u8
    out q   : u8

    reg r : u8 = 0
    r = a
    q = r
}

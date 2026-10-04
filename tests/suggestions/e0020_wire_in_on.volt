// suggestion: E0020
// A wire written with '<=' in an 'on' block holds a value between edges:
// the fix declares it as a register.
module W {
    in  clk : clock
    in  a   : u8
    out q   : u8

    wire w : u8
    on clk {
        w <= a
    }
    q = w
}

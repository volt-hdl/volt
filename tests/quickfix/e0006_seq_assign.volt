// quickfix: E0006
// A sequential block assigns with '<='; the fix replaces '='.
module Count {
    in  clk : clock
    out q   : u8

    reg c : u8 = 0
    on clk {
        c = c + 1
    }
    q = c
}

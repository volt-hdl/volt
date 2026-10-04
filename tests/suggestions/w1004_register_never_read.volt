// suggestion: W1004
// A register written but never read: the fix renames the declaration and
// every write, so no write is left pointing at the old name.
module U {
    in  clk : clock
    in  a   : u8
    out y   : u8

    reg q : u8 = 0
    on clk {
        q <= a
    }
    y = a
}

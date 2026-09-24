// parity: E1013
module T {
    in  clk    : clock
    in  packed : u8
    out y      : u8
    reg table : u8 = 0
    on clk { table <= packed }
    y = table
}

//~ E0006
// '=' used in a sequential block; it should be '<='.
// This is one of the most common Verilog mistakes.

module WrongAssign {
    in  clk : clock
    out y   : u8

    reg r : u8 = 0

    on clk {
        r = r + 1
        //~^ ERROR '=' cannot be used in a sequential block
    }

    y = r
}

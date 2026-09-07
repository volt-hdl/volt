//~ E2010
// The literal does not fit the target type.

module LiteralOverflow {
    in  clk : clock
    out r   : u8

    reg value : u8 = 300
    //~^ ERROR literal 300 does not fit in type u8 (maximum 255)

    on clk { value <= value }
    r = value
}

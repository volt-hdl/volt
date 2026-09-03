//~ E2010
// Literal hedef tipe sığmıyor.

module LiteralOverflow {
    in  clk : clock
    out r   : u8

    reg value : u8 = 300
    //~^ ERROR literal 300, u8 tipine sığmıyor (maksimum 255)

    on clk { value <= value }
    r = value
}

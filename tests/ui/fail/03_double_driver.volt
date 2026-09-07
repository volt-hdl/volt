//~ E4001
// The same signal is assigned from two different places.

module DoubleDriver {
    in  a : u8
    in  b : u8
    out y : u8

    y = a
    y = b
    //~^ ERROR 'y' is already driven (line 8)
}

//~ E2006
// Bit index out of bounds.

module IndexOutOfBounds {
    in  data : u8
    out bit  : bool

    bit = data[9]
    //~^ ERROR index 9 out of bounds (width 8)
}

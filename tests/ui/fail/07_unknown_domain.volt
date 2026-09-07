//~ E3002
// Reference to an undefined domain.

module UnknownDomain {
    in  clk  : clock @Nonexistent
    //~^ ERROR undefined clock domain: 'Nonexistent'
    out y    : u8

    y = 0
}

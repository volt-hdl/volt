//~ E0007
// '<=' outside an 'on' block. At module level `r <= r + 1` used to parse
// as a comparison and was dropped without an error; the register never
// counted.

module ModuleLevelNonblocking {
    in  clk : clock
    out q   : u8

    reg r : u8 = 0
    r <= r + 1
    //~^ ERROR '<=' cannot be used outside an 'on' block
    q = r
}

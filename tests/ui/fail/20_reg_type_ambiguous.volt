//~ E2012
// Register type cannot be determined -- literal initializer, no type.

module RegTypeAmbiguous {
    in  clk : clock
    out r   : u8

    reg counter = 0
    //~^ ERROR cannot determine register type

    on clk { counter <= counter + 1 }
    r = counter
}

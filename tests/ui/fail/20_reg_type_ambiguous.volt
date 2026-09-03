//~ E2012
// Register tipi belirlenemiyor — literal başlangıç, tip yok.

module RegTypeAmbiguous {
    in  clk : clock
    out r   : u8

    reg counter = 0
    //~^ ERROR register tipi belirlenemiyor

    on clk { counter <= counter + 1 }
    r = counter
}

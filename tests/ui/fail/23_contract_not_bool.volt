//~ E5004
// Kontrat ifadesi Bool tipinde olmalı (F4a ADIM 1).

module ContractNotBool {
    in  clk   : clock
    in  speed : u8
    out r     : u8

    requires: speed + 1
    //~^ ERROR kontrat ifadesi Bool değil
    r = speed
}

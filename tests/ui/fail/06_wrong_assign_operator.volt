//~ E0006
// Sıralı blokta '=' kullanıldı, '<=' olmalı.
// Bu Verilog'un en yaygın hatalarından biri.

module WrongAssign {
    in  clk : clock
    out y   : u8

    reg r : u8 = 0

    on clk {
        r = r + 1
        //~^ ERROR sıralı blokta '=' kullanılamaz
    }

    y = r
}

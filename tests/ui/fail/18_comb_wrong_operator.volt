//~ E0007
// Kombinasyonel blokta '<=' kullanıldı, '=' olmalı.

module CombWrongOperator {
    in  a : u8
    in  b : u8
    out r : u8

    comb {
        r <= a + b
        //~^ ERROR kombinasyonel blokta '<=' kullanılamaz
    }
}

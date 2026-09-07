//~ E0007
// '<=' used in a combinational block; it should be '='.

module CombWrongOperator {
    in  a : u8
    in  b : u8
    out r : u8

    comb {
        r <= a + b
        //~^ ERROR '<=' cannot be used in a combinational block
    }
}

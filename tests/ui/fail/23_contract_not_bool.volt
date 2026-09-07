//~ E5004
// The contract expression must have type Bool (F4a STEP 1).

module ContractNotBool {
    in  clk   : clock
    in  speed : u8
    out r     : u8

    requires: speed + 1
    //~^ ERROR contract expression is not Bool
    r = speed
}

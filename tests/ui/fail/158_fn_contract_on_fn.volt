//~ E0003
// ADR-0081 Karar 8: fn kontratı bu turda eşlenmez.
fn div(a: u8, b: u8) -> u8
    requires: b != 0
//~^ ERROR E0003 not supported yet: contracts on functions
{
    a / b
}

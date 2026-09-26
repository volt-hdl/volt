//~ E2016
// ADR-0081 Karar 1: fn durum tutamaz — reg fn gövdesinde yok.
fn acc(x: u8) -> u8 {
    reg s : u8 = 0
//~^ ERROR E2016 a register is not allowed in a function body
    x
}

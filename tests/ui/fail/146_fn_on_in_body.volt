//~ E2016
// ADR-0081 Karar 1: saat kenarı bloğu fn gövdesinde yok.
fn f(x: u8) -> u8 {
    on clk { }
//~^ ERROR E2016 an 'on' block is not allowed in a function body
    x
}

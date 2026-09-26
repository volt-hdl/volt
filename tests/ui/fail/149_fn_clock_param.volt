//~ E2016
// ADR-0081 Karar 4: saat parametresi fn'i bir saat alanına bağlar.
fn f(clk: clock, x: u8) -> u8 {
//~^ ERROR E2016 a clock cannot appear in a function signature
    x
}

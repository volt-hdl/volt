//~ E2003
// ADR-0081 Karar 5: argüman sayısı imzayla aynı olmalı.
fn inc(a: u8) -> u8 {
    a + 1
}

module Arity {
    in  a : u8
    out y : u8
    y = inc(a, a)
//~^ ERROR E2003 function 'inc' takes 1 argument(s), 2 given
}

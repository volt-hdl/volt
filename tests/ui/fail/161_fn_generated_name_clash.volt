//~ E1003
// ADR-0081 Karar 12.2: açılımın ürettiği tel adı modüldeki bir adla
// çakışıyor (inc_0).
fn inc(a: u8) -> u8 {
    a + 1
}

module Clash {
    in  a : u8
    out y : u8
    let inc_0 = a
    y = inc(a + 2) + inc_0
//~^ ERROR E1003 generates the signal 'inc_0'
}

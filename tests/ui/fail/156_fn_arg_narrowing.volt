//~ E2001
// ADR-0081 Karar 5: argüman parametre tipine check edilir; daraltma 'as'
// ister (atama ve port bağlantısıyla aynı kural).
fn low(a: u8) -> u8 {
    a
}

module Narrow {
    in  w : u16
    out y : u8
    y = low(w)
//~^ ERROR E2001 u16 to u8
}

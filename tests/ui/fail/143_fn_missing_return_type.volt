//~ E2015
// ADR-0081 Karar 2: fn'in sonucu yok — dönüş tipi yazılmamış.
fn parity(x: u8) {
//~^ ERROR E2015 function 'parity' has no return type
    x[0]
}

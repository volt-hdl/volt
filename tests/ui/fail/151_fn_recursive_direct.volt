//~ E4013
// ADR-0081 Karar 6: doğrudan özyineleme — her çağrı açılır, dibi yok.
fn f(x: u8) -> u8 {
//~^ ERROR E4013 function 'f' calls itself
    f(x) + 1
}

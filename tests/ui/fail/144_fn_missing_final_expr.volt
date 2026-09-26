//~ E2015
// ADR-0081 Karar 2: gövde son ifadeyle bitmiyor; let yalnız ara değerdir.
fn inc(a: u8) -> u8 {
    let t = a + 1
}
//~^ ERROR E2015 function 'inc' does not end with a final expression

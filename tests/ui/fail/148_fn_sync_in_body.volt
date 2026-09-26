//~ E2016
// ADR-0081 Karar 1: sync() register zinciri kurar — fn'de yok.
fn f(x: u8, c: u8) -> u8 {
    sync(x, c)
//~^ ERROR E2016 sync() is not allowed in a function body
}

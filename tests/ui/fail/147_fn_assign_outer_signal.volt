//~ E2016
// ADR-0081 Karar 1: fn hiçbir sinyale atama yapamaz (yan etki yok).
fn f(x: u8) -> u8 {
    y = x
//~^ ERROR E2016 an assignment is not allowed in a function body
    x
}

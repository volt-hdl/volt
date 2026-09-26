//~ E4013
// ADR-0081 Karar 6: karşılıklı özyineleme; döngüdeki her fn raporlanır.
fn g(x: u8) -> u8 {
    h(x) + 1
}

fn h(x: u8) -> u8 {
//~^ ERROR E4013 function 'h' calls itself (g → h → g)
    g(x)
}

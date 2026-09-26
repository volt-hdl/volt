//~ E0003
// ADR-0081 Karar 3: fn gövdesinde for (katlama anlamı) bu turda yok.
fn f(a: u8) -> u8 {
    for i in 0..4 {
//~^ ERROR E0003 not supported yet: 'for' loops in function bodies
        let t = a
    }
    a
}

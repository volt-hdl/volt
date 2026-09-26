//~ E2003
// ADR-0081 Karar 2: son ifade dönüş tipine check edilir.
fn is_small(a: u8) -> bool {
    a
//~^ ERROR E2003 type mismatch: expected 'bool', found 'u8'
}

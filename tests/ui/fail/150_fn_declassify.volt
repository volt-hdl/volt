//~ E3015
// ADR-0081 Karar 10: declassify fn gövdesinde her çağrıda görünmez bir
// güven düşürme olurdu; çağıran modülde, sonuca yazılır.
fn reveal(k: u8) -> u8 {
    declassify(k, "debug")
//~^ ERROR E3015 declassify cannot be used inside a function body
}

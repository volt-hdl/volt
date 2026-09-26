//~ E0003
// ADR-0081 Karar 12.3: ikame kipinde (comb) bitleri seçilen let'in yerine
// seçilebilir bir ifade gerekir; tel kurulamaz.
fn carry(a: u8, b: u8) -> bool {
    let s : u9 = a + b
    s[8]
}

module Carry {
    in  a : u8
    in  b : u8
    out c : bool
    comb {
        c = carry(a, b)
//~^ ERROR E0003 which selects bits of its let 's'
    }
}

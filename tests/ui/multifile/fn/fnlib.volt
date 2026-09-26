package fnlib;

// ADR-0081 + ADR-0042: pub fn başka dosyadan çağrılır; açılım çağıran
// modülde yapılır (fn SV'de görünmez).
pub fn sat_inc(a: u8) -> u8 {
    if a == 255 { a } else { a + 1 }
}

// Not `pub`: importing it from another file is E1004.
fn hidden(a: u8) -> u8 {
    a
}

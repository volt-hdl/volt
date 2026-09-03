//~ E2002
// İşaretli ve işaretsiz karıştırılamaz.
// type-inference.md §3.3

module SignednessMismatch {
    in  a : u8
    in  b : i8
    out r : i16

    r = (a + b) as i16
    //~^ ERROR işaretli ve işaretsiz karıştırılamaz
}

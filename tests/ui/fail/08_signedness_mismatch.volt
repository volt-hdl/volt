//~ E2002
// Signed and unsigned cannot be mixed.
// type-inference.md section 3.3

module SignednessMismatch {
    in  a : u8
    in  b : i8
    out r : i16

    r = (a + b) as i16
    //~^ ERROR cannot mix signed and unsigned
}

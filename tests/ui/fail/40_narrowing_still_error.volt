//~ E2001
// ADR-0041 permits widening under an explicit target type; implicit
// narrowing remains an error (type-inference.md sec. 5).

module Narrowing {
    in  u : i16
    out v : i8

    let t : i8 = u
    //~^ ERROR E2001

    v = t
}

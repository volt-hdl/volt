//~ E2002
// ADR-0041 relaxes same-sign widening only; a signed value still does
// not flow into an unsigned target, even a wider one.

module SignMismatch {
    in  w : i16
    out z : u32

    let t : u32 = w
    //~^ ERROR E2002

    z = t
}

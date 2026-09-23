//~ E4009
// A plain struct that contains itself has no finite bit width: 'P' would
// need 8 + width(P) bits. Before ADR-0069 'volt check' accepted it without
// a diagnostic (only 'volt build' failed, with an unrelated E0003).
struct P {
//~^ ERROR E4009
    d : u8
    f : P
}

module M {
    in  a : P
    out y : u8

    y = a.d
}

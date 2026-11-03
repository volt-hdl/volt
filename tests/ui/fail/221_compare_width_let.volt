//~ E2031
// An untyped let keeps the carry, so the same comparison through a let
// is the same error as the inline form (ADR-0106).
module CompareLet {
    in  a  : u3
    in  b  : u3
    out eq : bool

    let s = b + 1
    eq = a == s
    //~^ ERROR E2031
}

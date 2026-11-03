//~ E2031
// `b + 1` keeps its carry (4 bits); compared with the 3-bit `a`, the
// comparison could wrap at 3 bits or keep the carry. Volt does not
// choose (ADR-0106).
module CompareInline {
    in  a  : u3
    in  b  : u3
    out eq : bool

    eq = a == b + 1
    //~^ ERROR E2031
}

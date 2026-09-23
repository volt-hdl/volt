//~ E0003
// A port group is flattened to plain ports at parse time (ADR-0039) and
// no generic parameter is substituted there -- only modules take (const)
// generic arguments (ADR-0041). A generic struct port used to be accepted
// and left unflattened: 'g.d' passed 'volt check' without a diagnostic.
struct port G<T> {
//~^ ERROR E0003
    out d : T
}

module M {
    in  g : G<u8>
    out o : u8

    o = g.d
}

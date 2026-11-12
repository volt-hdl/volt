//~ E2010
// #213: -500 does not fit in i8, the type of the other side (ADR-0106). It
// was written 8'shC (i < 12).
module M {
    in  i : i8
    out c : bool
    c = i < -500
}

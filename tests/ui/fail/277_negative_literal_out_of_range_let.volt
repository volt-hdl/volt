//~ E2010
// #213: -129 does not fit in i8 (-128..=127). It was written 8'sh7F (+127).
module M {
    out b : i8
    b = -129
}

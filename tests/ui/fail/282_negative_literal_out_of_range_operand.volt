//~ E2010
// #213: -300 does not fit in i8, the type of the other operand. It was
// written 8'shD4 (i - 44).
module M {
    in  i : i8
    out f : i9
    f = i + -300
}

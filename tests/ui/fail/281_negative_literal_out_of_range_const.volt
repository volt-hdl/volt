//~ E2010
// #213: -9 does not fit in i4 (-8..=7). It was written 4'sh7 (+7).
const K : i4 = -9
module M {
    out o : i4
    o = K
}

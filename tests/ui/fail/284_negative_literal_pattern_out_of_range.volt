//~ E2010
// #213: the pattern -5 does not fit in i3 (-4..=3). It was written
// 3'sh3 and fired for i == 3.
module M {
    in  i : i3
    out o : bool
    o = match i {
        -5 => true,
        _ => false,
    }
}

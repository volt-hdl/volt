//~ E2010
// #213: -1 does not fit in u8 (0..=255). It was written -8'd1 (255).
module M {
    out a : u8
    a = -1
}

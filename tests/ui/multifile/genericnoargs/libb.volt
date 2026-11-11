package libb;

// #249: the generic module M_1 needs one generic argument.
pub module M_1<const C: u32> {
    in  x : u8
    out y : u8
    y = x ^ C
}

package libb;

// #205: M_1<8> builds the module name 'M_1_8' as well.
pub module M_1<const C: u32> {
    in  x : u8
    out y : u8
    y = x ^ C
}

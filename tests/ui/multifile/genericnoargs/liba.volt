package liba;

// #249: M<1> builds the module name 'M_1'.
pub module M<const A: u32> {
    in  x : u8
    out y : u8
    y = x + A
}

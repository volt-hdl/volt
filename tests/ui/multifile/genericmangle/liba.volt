package liba;

// #205: M<1, 8> builds the module name 'M_1_8'.
pub module M<const A: u32, const B: u32> {
    in  x : uint<B>
    out y : uint<B>
    y = x + A
}

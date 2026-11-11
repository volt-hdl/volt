//~ E1003
// #205: a generic instance gets the module name '<module>_<arguments>'
// (sv-mapping.md §9). M<1, 8> and M_1<8> are two different generic modules
// that both build 'M_1_8'. Volt does not rename generated SystemVerilog
// names (ADR-0090): E1003, and one of the two modules gets a new name.
// Before the fix only 'M_1_8.sv' was written (the body of M), and 'q'
// computed x + 1 instead of x ^ 8.
module M<const A: u32, const B: u32> {
    in  x : uint<B>
    out y : uint<B>
    y = x + A
}

module M_1<const C: u32> {
    in  x : u8
    out y : u8
    y = x ^ C
}

module Top {
    in  x : u8
    out a : u8
    out b : u8
    let p = M<1, 8> { x: x }
    let q = M_1<8> { x: x }
    //~^ ERROR 'M_1_8' is the generated module name of both M<1, 8> and M_1<8>
    a = p.y
    b = q.y
}

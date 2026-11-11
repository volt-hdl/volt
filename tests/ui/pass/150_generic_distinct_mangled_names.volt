// #205, the safe twin of fail/265: M<1, 8> builds 'M_1_8' and M_1<7>
// builds 'M_1_7'. The two generic modules build different module names,
// so each instance keeps its own body: 'a' is x + 1 and 'b' is x ^ 7.
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
    let q = M_1<7> { x: x }
    a = p.y
    b = q.y
}

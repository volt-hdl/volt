//~ E1003
// #205, the twin of 265 in the other order: the templates and the
// instances are written the other way round. The collision does not
// depend on which instance is expanded first; the message names the two
// instances in source order and points at the later one.
module M_1<const C: u32> {
    in  x : u8
    out y : u8
    y = x ^ C
}

module M<const A: u32, const B: u32> {
    in  x : uint<B>
    out y : uint<B>
    y = x + A
}

module Top {
    in  x : u8
    out a : u8
    out b : u8
    let q = M_1<8> { x: x }
    let p = M<1, 8> { x: x }
    //~^ ERROR 'M_1_8' is the generated module name of both M_1<8> and M<1, 8>
    a = p.y
    b = q.y
}

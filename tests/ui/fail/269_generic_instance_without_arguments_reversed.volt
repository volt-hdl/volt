//~ E2003
// #249, fail/268 in the other order: the instance without generic
// arguments comes first, and the generic module M_1 is declared before M.
// The rule does not depend on the order: E2003 at the instance 'r'.
module M_1<const C: u32> {
    in  x : u8
    out y : u8
    y = x ^ C
}

module M<const A: u32> {
    in  x : u8
    out y : u8
    y = x + A
}

module Top {
    in  x : u8
    out a : u8
    out b : u8
    out c : u8
    let r = M_1 { x: x }
    //~^ ERROR 'M_1' expects 1 generic argument(s), got 0
    let q = M_1<5> { x: x }
    let p = M<1> { x: x }
    a = p.y
    b = q.y
    c = r.y
}

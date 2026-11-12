//~ E2003
// #249, the silent wrong form: M<1> builds the module name 'M_1', and the
// generic module M_1 leaves the module list because M_1<5> instantiates
// it. Before the fix the instance 'r' without generic arguments was bound
// to the module 'M_1', which is the body of M<1>: 'c' computed x + 1, and
// nothing was reported. An instance of a generic module needs one
// argument per generic parameter (ADR-0041): E2003.
module M<const A: u32> {
    in  x : u8
    out y : u8
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
    out c : u8
    let p = M<1> { x: x }
    let q = M_1<5> { x: x }
    let r = M_1 { x: x }
    //~^ ERROR 'M_1' expects 1 generic argument(s), got 0
    a = p.y
    b = q.y
    c = r.y
}

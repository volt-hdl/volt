//~ E2003
// #249: an instance of a generic module without generic arguments was
// accepted. M<1> builds the module name 'M_1' (sv-mapping.md §9, generic
// instance names), the generic module M_1 itself leaves the module list
// because M_1<5> instantiates it, and the bare `M_1 { x: x }` was bound
// to the body of M<1>: `c` computed x + 1 instead of anything of M_1, with
// no error (only a misleading W1006). Right answer: generic parameters
// have no default values (grammar-full.ebnf GenericParam), so the instance
// gives 0 arguments for 1 parameter: E2003 by the argument count rule of
// ADR-0041 (sv-mapping.md §9, #249), and no W1006. Fixed by #260.
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
    a = p.y
    b = q.y
    c = r.y
}

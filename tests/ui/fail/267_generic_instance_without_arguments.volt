//~ E2003
// #249: generic parameters have no default values (the grammar's
// GenericParam has no '= value'), so an instance of a generic module
// without generic arguments has 0 arguments for 1 parameter: E2003
// (ADR-0041, the argument count rule). Before the fix it was accepted:
// 'volt build' exited 0 and wrote a Top.sv that instantiates 'M_1', a
// module that is never generated, and the only diagnostic was a W1006
// saying that 'M_1' is never instantiated.
module M_1<const C: u32> {
    in  x : u8
    out y : u8
    y = x ^ C
}

module Top {
    in  x : u8
    out b : u8
    let q = M_1 { x: x }
    //~^ ERROR 'M_1' expects 1 generic argument(s), got 0
    b = q.y
}

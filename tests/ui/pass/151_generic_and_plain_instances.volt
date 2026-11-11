// #249, the safe twin of fail/267-269: the generic module M is
// instantiated with its argument (M<1> builds 'M_1'), and the module M_2,
// which is not generic, is instantiated without arguments. Each instance
// keeps its own body: 'a' is x + 1 and 'b' is x ^ 2.
module M<const A: u32> {
    in  x : u8
    out y : u8
    y = x + A
}

module M_2 {
    in  x : u8
    out y : u8
    y = x ^ 2
}

module Top {
    in  x : u8
    out a : u8
    out b : u8
    let p = M<1> { x: x }
    let q = M_2 { x: x }
    a = p.y
    b = q.y
}

// #130: the parser substitutes generic parameters (ADR-0041) and
// module-level `for` variables (ADR-0056) before name resolution, through
// the scope chain of name-resolution.md §2: an inner declaration with the
// same name shadows them (W1002, §6) and its uses read the inner binding.
// Here no name is reused: the generic N, the module-level loop variable i
// and the body let p reach every position (index, part-select start,
// generic argument, shorthand port binding, pattern, nested loop).

module Stage<const K: u32> {
    in  a : u8
    out y : u8
    y = a ^ K
}

module Scopes<const N: u32> {
    in  xs : [u8; 4]
    in  s  : u8
    out ys : [u8; 4]
    out zs : [u2; 2]
    out m  : u8

    for i in 0..N {
        let p = Stage<i> { a: xs[i] }
        ys[i] = p.y
    }
    for j in 0..2 {
        zs[j] = s[j * N +: 2] as u2
    }
    m = match s {
        N => 1,
        _ => 0,
    }
}

module GenericAndModuleForScopes {
    in  xs : [u8; 4]
    in  s  : u8
    out ys : [u8; 4]
    out zs : [u2; 2]
    out m  : u8

    let u = Scopes<4> { xs: xs, s: s }
    ys = u.ys
    zs = u.zs
    m = u.m
}

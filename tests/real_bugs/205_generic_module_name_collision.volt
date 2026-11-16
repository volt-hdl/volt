//~ E1003
// #205: a generic instance gets the module name `<module>_<arguments>`
// (sv-mapping.md §9). M<1, 8> and M_1<8> are two different generic
// modules that both build `M_1_8`. The monomorphization table was keyed by
// that name alone, so only one `M_1_8` (the body of M) was written and
// both instances were bound to it: `q` computed x + 1 instead of x ^ 8,
// with only a misleading W1006. ADR-0090: Volt never renames a generated
// SystemVerilog name by itself; a collision of generated names is E1003.
// Fixed in PR #252.
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
    a = p.y
    b = q.y
}

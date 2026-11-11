// suggestion: E2001
// #204: the narrowing fix on a shorthand field keeps the field name:
// `S { a }` becomes `S { a: a[7:0] as u8 }`, not `S { a[7:0] as u8 }`.
struct S { a : u8 }

module M {
    in  a : u16
    out o : S

    o = S { a }
}

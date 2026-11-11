//~ E2001
// #204: the field-init shorthand `S { a }` means `S { a: a }`
// (grammar-full.ebnf FieldInit). A 16-bit input in an 8-bit field is
// a width error in both forms (type-inference.md: no implicit
// narrowing).
struct S { a : u8 }

module M {
    in  a : u16
    out o : S
    o = S { a }
    //~^ ERROR a 16-bit value does not fit in an 8-bit target
}

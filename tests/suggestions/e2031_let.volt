// suggestion: E2031
// The same comparison through an untyped let, which keeps the carry.
module CmpLet {
    in  a  : u3
    in  b  : u3
    out eq : bool

    let s = b + 1
    eq = a == s
}

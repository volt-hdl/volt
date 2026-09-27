// parity: E1003
module A {
    out c : bool
    c = true
}
module B {
    out b_c : bool
    b_c = false
}
module M {
    out o : bool
    let a_b = A { }
    let a = B { }
    o = a_b.c ^ a.b_c
}

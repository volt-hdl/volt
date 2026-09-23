//~ E4009
// Mutual recursion: A contains B, B contains A. Every type on the cycle is
// reported, each with the field that closes the cycle and the cycle path
// ('A.b -> B.a -> A').
struct A {
//~^ ERROR E4009
    b : B
}

struct B {
//~^ ERROR E4009
    a : A
}

module M {
    in  x : A
    out y : u8

    y = 0
}

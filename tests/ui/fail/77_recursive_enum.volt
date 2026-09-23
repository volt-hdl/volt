//~ E4009
// An enum with a payload is as wide as its largest variant, so a variant
// that carries the enum itself (directly, through a struct variant, through
// the base type or through another type) makes it infinitely wide.
enum E {
//~^ ERROR E4009
    Idle
    Wrap(E)
}

enum F {
//~^ ERROR E4009
    Idle
    Node { next : F }
}

enum G : G {
//~^ ERROR E4009
    Idle
}

enum H {
//~^ ERROR E4009
    Idle
    Boxed(S)
}

struct S {
//~^ ERROR E4009
    h : H
}

module M {
    in  a : u8
    out y : u8

    y = a
}

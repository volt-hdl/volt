//~ E4009
// A struct port may reach itself through a plain struct or through an
// array field; both were flattened without a diagnostic before ADR-0069.
struct port A {
//~^ ERROR E4009
    out p : P
}

struct P {
//~^ ERROR E4009
    a : A
}

struct port S {
//~^ ERROR E4009
    out d : u8
    out x : [S; 2]
}

module M {
    in  a : A
    in  s : S
    out y : u8

    y = 0
}

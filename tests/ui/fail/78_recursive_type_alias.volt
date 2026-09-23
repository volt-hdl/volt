//~ E4009
// A type alias is only another name for its target; an alias whose target
// leads back to the alias never resolves to a real type.
type T = T
//~^ ERROR E4009

type U = V
//~^ ERROR E4009
type V = U
//~^ ERROR E4009

type A = [S; 2]
//~^ ERROR E4009

struct S {
//~^ ERROR E4009
    a : A
}

module M {
    in  a : u8
    out y : u8

    y = a
}

//~ E4009
// Recursion through an array or a tuple is still recursion: '[S; 4]' is
// four copies of S, '(u8, T)' holds a whole T.
struct S {
//~^ ERROR E4009
    x : [S; 4]
}

struct T {
//~^ ERROR E4009
    t : (u8, T)
}

module M {
    in  s : S
    in  t : T
    out y : u8

    y = 0
}

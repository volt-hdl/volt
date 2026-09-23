//~ E4009
// Generic structs are checked on their declarations: 'W<T>' contains
// 'W<T>' for every T, 'G<T>' contains an ever-growing 'G<G<T>>', and
// 'P' contains itself through 'Box<P>' because Box stores its parameter.
// 'Box' itself is finite and is not reported.
struct W<T> {
//~^ ERROR E4009
    x : W<T>
}

struct G<T> {
//~^ ERROR E4009
    x : G<G<T>>
}

struct Box<T> {
    v : T
}

struct P {
//~^ ERROR E4009
    b : Box<P>
}

module M {
    in  a : u8
    out y : u8

    y = a
}

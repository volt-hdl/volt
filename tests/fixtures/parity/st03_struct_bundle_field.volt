// parity: E2013
struct port B {
    out d : u4
}
struct S {
    x : B
}
module M {
    in  x : u4
    out y : u4
    y = x
}

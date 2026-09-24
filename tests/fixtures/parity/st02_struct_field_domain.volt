// parity: E2013
domain D {
    clock = posedge
}
struct S {
    a : u4 @D
}
module M {
    in  x : u4
    out y : u4
    y = x
}

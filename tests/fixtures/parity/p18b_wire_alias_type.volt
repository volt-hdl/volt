// parity: ok
type W = u8
module M {
    in  x : u8
    out y : u8
    wire w : W
    w = x
    y = w
}

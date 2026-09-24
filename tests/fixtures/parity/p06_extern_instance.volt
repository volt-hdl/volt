// parity: E0003
extern module Ext {
    in  d : u8
    out q : u8
}
module M {
    in  x : u8
    out y : u8
    let e = Ext { d: x }
    y = e.q
}

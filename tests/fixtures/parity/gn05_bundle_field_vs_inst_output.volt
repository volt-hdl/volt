// parity: E1003
struct port Handshake {
    out data  : u8
    out valid : bool
    in  ready : bool
}
module Src {
    out data : u8
    data = 1
}
module M {
    in  hs : Handshake
    out o  : u8
    let hs = Src { }
    hs.ready = true
    o = hs.data
}

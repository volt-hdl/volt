//~ E1003
// ADR-0090: the bundle port 'hs' flattens to 'hs_data' (ADR-0039) and the
// instance 'hs' has an output 'data' — the same SystemVerilog name.

struct port Handshake {
    out data  : u8
    out valid : bool
    in  ready : bool
}

module Src {
    out data : u8
    data = 1
}

module Sink {
    in  hs : Handshake
    out o  : u8

    let hs = Src { }
//~^ ERROR 'hs_data' is both field 'data' of bundle port 'hs' and output 'data' of instance 'hs'
    hs.ready = true
    o = hs.data
}

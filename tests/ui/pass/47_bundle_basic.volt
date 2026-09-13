// Bundle ports (ADR-0039): one 'struct port' definition shared by the
// producer ('out hs') and the consumer ('in hs'). The parser flattens
// each bundle to <port>_<field> ports; 'in' flips every field direction,
// so hs.ready is an output of Consumer and an input of Producer.
struct port Handshake {
    out data  : u8
    out valid : bool
    in  ready : bool
}

module Producer {
    in  clk : clock
    out hs  : Handshake

    cover: hs.valid && hs.ready

    reg count : u8 = 0
    on clk {
        if hs.ready {
            count <= count + 1
        }
    }
    hs.data = count
    hs.valid = true
}

module Consumer {
    in  clk : clock
    in  hs  : Handshake
    out sum : u8

    reg acc : u8 = 0
    on clk {
        if hs.valid {
            acc <= acc + hs.data
        }
    }
    hs.ready = true
    sum = acc
}

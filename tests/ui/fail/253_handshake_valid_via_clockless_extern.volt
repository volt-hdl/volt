//~ E4007
// ADR-0114 K4: an extern without a clock is combinational and its body
// is not visible, so y may depend on b = tx.ready. When the real extern
// has no such path, the help names @no_protocol_check (see
// tests/ui/pass/147).
extern module AndExt {
    in  a : bool
    in  b : bool
    out y : bool
}

module Producer {
    in  clk  : clock
    in  have : bool
    out tx   : Handshake<u8>

    reg count : u8 = 0
    on clk {
        if tx.fired {
            count <= count + 1
        }
    }
    let g = AndExt { a: have, b: tx.ready }
    tx.valid = g.y
    //~^ ERROR handshake 'tx': valid depends combinationally on ready
    tx.data  = count
}

// Output net (ADR-0079):
//~ NET-SKIP: the extern has no @source body on purpose (the fixture shows the handshake rule); the design is incomplete without external SV

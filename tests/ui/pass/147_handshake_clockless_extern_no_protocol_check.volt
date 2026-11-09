// ADR-0114 K4: a clockless extern is assumed to connect every output to
// every input. When its paths are independent (here y follows a only),
// the way out of the false E4007 is @no_protocol_check on the port: the
// check then ignores paths that exist only by that assumption (a visible
// path is still E4007), and the automatic contracts are off (ADR-0050).
extern module Pair {
    in  a : bool
    in  b : bool
    out y : bool
    out z : bool
}

module Producer {
    in  clk  : clock
    in  have : bool
    out busy : bool
    @no_protocol_check
    out tx   : Handshake<u8>

    reg count : u8 = 0
    on clk {
        if tx.fired {
            count <= count + 1
        }
    }
    let p = Pair { a: have, b: tx.ready }
    tx.valid = p.y
    busy     = p.z
    tx.data  = count
}

// Output net (ADR-0079):
//~ NET-SKIP: the extern has no @source body on purpose (the fixture shows the handshake rule); the design is incomplete without external SV

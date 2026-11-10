//~ E4007
// ADR-0114 K4: a clockless extern is assumed to connect every output to
// every input, so p.y may depend on b = tx.ready. A path that exists only
// by that assumption is E4007 too; the note says it rests on the
// assumption and the help decides valid from registered state.
// @no_protocol_check does not change E4007: it only turns off the
// automatic protocol contracts of the port (ADR-0050 §3).
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
    //~^ ERROR handshake 'tx': valid depends combinationally on ready
    busy     = p.z
    tx.data  = count
}

// Output net (ADR-0079):
//~ NET-SKIP: the extern has no @source body on purpose (the fixture shows the handshake rule); the design is incomplete without external SV

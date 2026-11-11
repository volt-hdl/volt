//~ E4007
// #193, ADR-0114: the path from valid to ready goes through a submodule.
// The child's summary says y depends on b, b is bound to tx.ready: valid
// waits for ready (AXI A3.3.1).
module AndGate {
    in  a : bool
    in  b : bool
    out y : bool
    y = a && b
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
    let g = AndGate { a: have, b: tx.ready }
    tx.valid = g.y
    //~^ ERROR handshake 'tx': valid depends combinationally on ready
    tx.data  = count
}

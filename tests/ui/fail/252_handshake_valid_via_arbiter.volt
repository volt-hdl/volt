//~ E4007
// ADR-0114 K7: an arbiter's grant is combinational in req.
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
    wire req : bits<2>
    req[0] = have && tx.ready
    req[1] = false
    let arb = PriorityArbiter<2> { clk: clk, req: req }
    tx.valid = arb.grant[0]
    //~^ ERROR handshake 'tx': valid depends combinationally on ready
    tx.data  = count
}

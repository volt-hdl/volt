//~ E2025
// RoundRobinArbiter N must be within 2..=64 (ADR-0029): a single
// requester needs no arbiter and the mask logic is bounded at 64.

module BadArbiter {
    in  clk   : clock
    in  req   : bits<1>
    out grant : bits<1>

    let rr = RoundRobinArbiter<1> {
    //~^ ERROR RoundRobinArbiter N is out of range
        clk: clk,
        req: req,
    }

    grant = rr.grant
}

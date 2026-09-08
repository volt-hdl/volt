// Built-in arbiters (ADR-0029). RoundRobinArbiter rotates priority
// after each grant so every requester is eventually served;
// PriorityArbiter is fixed priority with req[0] highest. Both prove
// `popcount(grant) <= 1` (one-hot or zero) and `grant & req == grant`.
module BusArbiter {
    in  clk      : clock
    in  req      : bits<4>
    out grant_rr : bits<4>
    out grant_fx : bits<4>

    let rr = RoundRobinArbiter<4> {
        clk: clk,
        req: req,
    }

    let fx = PriorityArbiter<4> {
        clk: clk,
        req: req,
    }

    grant_rr = rr.grant
    grant_fx = fx.grant
}

// Implication operator (ADR-0034): a -> b desugars to !a || b.
// Right-associative, lowest precedence (below ||); works both in
// contracts (SVA: a |-> b) and in RTL expressions (SV: !a || b).

module ImplicationOp {
    in  clk   : clock
    in  start : bool
    in  ready : bool
    out grant : bool
    out safe  : bool
    out busy  : bool

    requires:  start -> ready
    ensures:   start -> busy
    invariant: !busy_r -> !grant_r

    reg busy_r  : bool = false
    reg grant_r : bool = false

    on clk {
        busy_r  <= start
        grant_r <= start && ready
    }

    grant = grant_r
    busy = busy_r
    // RTL konumunda da geçerli: || implikasyondan sıkı bağlanır.
    safe = busy_r || ready -> start -> ready
}

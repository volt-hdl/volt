// @strict_timing with explicit Delayed stage types (ADR-0037): equal
// delays combine freely, a real stage register aligns a younger value
// (ADR-0127: s1_d2 holds s1 one cycle later, so both sides of 'both' are
// two cycles old), literals carry no timing, and an explicitly annotated
// let is a retiming assertion (the forwarding escape hatch) whose
// initializer may mix stages. Registers add one cycle: s1 (1) <= x (0),
// s2 (2) <= s1 (1), s1_d2 (2) <= s1 (1). delay<K> is a claim, not a
// register: tests/ui/pass/161_delay_is_a_claim.volt.

@strict_timing
module DelayedAligned {
    in  clk : clock
    in  x   : u32
    out y   : u32
    out fwd : u32
    out p1  : u32

    reg s1    : Delayed<u32, 1> = 0
    reg s2    : Delayed<u32, 2> = 0
    reg s1_d2 : Delayed<u32, 2> = 0

    let both  = s1_d2 + s2
    let plus5 = s1 + 5
    let asserted : Delayed<u32, 1> =
        if s2 != 0 { s2 } else { s1 }

    on clk {
        s1 <= x
        s2 <= s1
        s1_d2 <= s1
    }

    y   = both
    fwd = asserted
    p1  = plus5
}

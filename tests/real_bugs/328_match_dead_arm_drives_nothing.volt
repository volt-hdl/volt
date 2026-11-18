//~ W2014
//~ E4002
// #328: `c` is assigned only in the arm `1`, after the wildcard. The first
// matching arm wins (ADR-0083 Karar 5, const-eval.md) and arms after the
// first wildcard are not emitted and get W2014 (sv-mapping.md §19.1), so
// nothing drives `c`: E4002 (type-inference.md §11.3). The driver analysis
// counted the dead arm's assignment, `volt check` reported only W2014 and
// the generated SV left the output undriven (Z). The same gap gave a false
// E4001 when `c` had a driver outside the block.
module M {
    in  s : u2
    out a : u8
    out c : u8
    comb {
        match s {
            0 => { a = 1 }
            _ => { a = 7 }
            1 => { a = 9; c = 3 }
        }
    }
}

// The W0021 opt-out (ADR-0048): @allow(unenforced) on the same item
// acknowledges that the attribute is documentation for now. The
// attribute stays in the source so it starts working the day the
// compiler learns to enforce it. Silence is expected: zero diagnostics.
@budget(lut = 5000) @allow(unenforced)
module Passthrough {
    in  clk : clock
    in  a   : u8
    out y   : u8

    reg r : u8 = 0

    on clk {
        r <= a
    }

    y = r
}

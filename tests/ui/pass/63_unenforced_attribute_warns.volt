// An attribute that the compiler parses but does not enforce yet
// (ADR-0048). @timing is in the grammar, so it is not W0020, but no
// pass reads it and no SDC is written: the user would otherwise
// believe a constraint exists. W0021 makes that visible. The file is
// still a PASS fixture -- a warning, not an error.
@timing(clk = 100000000)
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

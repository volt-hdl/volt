// An attribute that the compiler parses but does not enforce yet
// (ADR-0048). @budget is in the grammar, so it is not W0020, but no
// pass reads it and no utilization check runs: the user would otherwise
// believe a check exists. W0021 makes that visible. The file is still a
// PASS fixture -- a warning, not an error. (@timing used to be the
// example here; it is enforced since ADR-0054 and no longer warns.)
@budget(lut = 5000)
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

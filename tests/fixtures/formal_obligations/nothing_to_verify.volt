// ADR-0097: the only contract is the top module's own `requires`, an
// assumption about the environment. Nothing is checked, so the run is
// not a success: E5006 (exit 1).

module Top {
    in  clk : clock
    in  x   : u8
    out y   : u8

    requires: x < 10

    reg r : u8 = 0
    on clk {
        r <= x
    }
    y = r
}

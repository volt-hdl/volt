//~ E0014
// A match statement without a wildcard arm (ADR-0032): until the F3
// exhaustiveness analysis lands, the `_` arm is mandatory.

module MatchNoWildcard {
    in  clk : clock
    out y   : bool

    reg state_r : u2   = 0
    reg y_r     : bool = false

    on clk {
        match state_r {
        //~^ ERROR 'match' statement has no '_' arm
            0 => { y_r <= true }
            1 => { y_r <= false }
        }
    }

    y = y_r
}

//~ E1015
// ADR-0085: only 'const' and generic parameters are pattern values; a
// loop variable is rejected like a signal.

module LoopVarPattern {
    in  clk : clock
    in  x   : u2
    out y   : u8

    reg r : [u8; 4] = [0; 4]
    on clk {
        for i in 0..4 {
            match x {
                i => { r[i] <= 1 }
                //~^ ERROR 'i' in a pattern must be a constant, but it is a loop variable
                _ => { }
            }
        }
    }
    y = r[0]
}

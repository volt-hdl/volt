// Single-clock rule: no domain is ever written
// domain-inference.md K2 -- UX Constitution principle
module SingleClock {
    in  clk    : clock
    in  enable : bool
    in  data   : u8
    out result : u8

    reg buffer : u8 = 0

    // No @Domain anywhere -- it is inferred
    on clk {
        if enable {
            buffer <= data
        }
    }

    result = buffer
}

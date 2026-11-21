//~ E5010
// @strict_timing: an undelayed input port (0 cycles by definition)
// cannot feed the same operator as a 2-cycle stage register (ADR-0037).
// The fix is two stage registers for x (ADR-0127); delay<2>(x) would
// only claim the age and add no register.

@strict_timing
module MissingDelay {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg tmp : Delayed<u32, 1> = 0
    reg b   : Delayed<u32, 2> = 0

    let sum = x + b
    //~^ ERROR timing misalignment

    on clk {
        tmp <= x
        b   <= tmp
    }

    y = sum
}

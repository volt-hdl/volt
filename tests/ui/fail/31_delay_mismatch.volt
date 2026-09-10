//~ E5010
// @strict_timing: combining a 1-cycle register with a 2-cycle register
// in one operator without re-alignment is a timing error (ADR-0037).

@strict_timing
module DelayMismatch {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = a + b
    //~^ ERROR timing misalignment

    on clk {
        a <= x
        b <= a
    }

    y = sum
}

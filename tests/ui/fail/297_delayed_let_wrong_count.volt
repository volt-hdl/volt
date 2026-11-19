//~ E5010
// #354: a declared Delayed<T, N> on a let is compared with the delay of
// its initializer when that delay is exactly known. `b` is 2 cycles old;
// the declaration says 1. Unchecked, the wrong count made `s + a` look
// aligned one line later, and the circuit added values of two cycles.

@strict_timing
module DelayedLetWrongCount {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    on clk {
        a <= x
        b <= a
    }

    let s : Delayed<u32, 1> = b
    //~^ ERROR declared delay does not match the value
    y = s + a
}

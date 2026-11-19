//~ E5010
// #354: a declared Delayed<T, N> on a wire is compared with the delay of
// its driver when that delay is exactly known: `b` is 2 cycles, not 1.

@strict_timing
module DelayedWireWrongCount {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    on clk {
        a <= x
        b <= a
    }

    wire w : Delayed<u32, 1>
    //~^ ERROR declared delay does not match the value
    w = b
    y = w + a
}

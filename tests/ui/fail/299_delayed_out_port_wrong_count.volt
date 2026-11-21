//~ E5010
// #354: the Delayed<T, N> of an output port is the module's promise at its
// boundary. It is compared with the delay of the driver: `a` is 1 cycle,
// not 5. The declaration was overwritten by inference without a word.

@strict_timing
module DelayedOutPortWrongCount {
    in  clk : clock
    in  x   : u32
    out y   : Delayed<u32, 5>
    //~^ ERROR declared delay does not match the value

    reg a : Delayed<u32, 1> = 0
    on clk {
        a <= x
    }
    y = a
}

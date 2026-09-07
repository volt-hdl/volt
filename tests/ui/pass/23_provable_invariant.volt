// Provable invariant (F4b): the counter wraps around at 10, so
// count_r <= 10 holds in every reachable state. 'volt verify'
// must pass this design with exit code 0 in bmc/prove mode.

module BoundedCounter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    invariant: count_r <= 10

    reg count_r : u8 = 0

    on clk {
        if enable {
            if count_r == 10 {
                count_r <= 0
            } else {
                count_r <= count_r + 1
            }
        }
    }

    count = count_r
}

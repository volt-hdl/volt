//~ E5001
// Violated invariant (F4b): even though the counter can climb to 10,
// the contract claims it stays below 5. This file passes COMPILATION --
// the violation is caught not by the compiler but by 'volt verify'
// (SymbiYosys counterexample, exit code 6); hence the expected code is E5001.

module LeakyCounter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    invariant: count_r < 5

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

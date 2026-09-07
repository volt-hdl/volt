/// 8-bit up counter
/// Increments on each clock edge while enable is high
module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    reg count_r : u8 = 0

    on clk {
        if enable {
            count_r <= count_r + 1
        }
    }

    count = count_r
}

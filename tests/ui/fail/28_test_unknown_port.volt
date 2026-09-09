//~ E8502
// Test statements may only touch declared ports (ADR-0033): 'enabel'
// is a typo for 'enable' and must be rejected at compile time.

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

test "typo in port name" {
    let dut = Counter { };
    dut.enabel = true;
    //~^ ERROR module 'Counter' has no port 'enabel'
    step(1);
}

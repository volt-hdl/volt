// ADR-0064 proof: a contract violation beyond the formal depth.
//
// The counter was meant to count 0..99 and wrap. The deliberate bug
// wraps at 100 instead, so `count` first reaches 100 at cycle 101 (the
// invariant is checked at every clock edge, en held high). A bounded
// check of 20 steps can reach at most 20:
//     volt verify --mode bmc --depth 20  -> passes (cannot see it)
//     volt test (200 cycles)             -> fails at cycle 101
module WrapCounter {
    in  clk   : clock
    in  en    : bool
    out count : u8

    invariant: count < 100
    cover: count == 50

    reg c : u8 = 0
    on clk {
        if en {
            // BUG (deliberate): must wrap at 99.
            if c == 100 {
                c <= 0
            } else {
                c <= c + 1
            }
        }
    }
    count = c
}

test "wraps_within_range" {
    let dut = WrapCounter { };
    dut.en = 1;
    step(200);
}

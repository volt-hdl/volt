//~ PASS
// #232: E5010's help and `volt explain E5010` advised `delay<1>(a) + b`
// to align a 1-cycle value with a 2-cycle one ("both sides are 2 cycles
// old"). Applied, the advice removed E5010 and the module compiled with
// no warning, but `delay<K>` adds no register (ADR-0037 §2,
// type-inference.md §12: it erases at the type level), so the circuit
// still added values of different cycles. ADR-0127: the help shows a real
// stage register first, with its code and cost, and names `delay<K>` last,
// as a claim Volt does not check. Fixed in the pull request of ADR-0127.
//
// `Claimed` is the old advice applied, `Aligned` the new one. Both compile
// clean; only `Aligned` aligns the values.

@strict_timing
module Claimed {
    in  clk : clock
    in  x   : u32
    out sum : u32
    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0
    on clk { a <= x  b <= a }
    sum = delay<1>(a) + b
}

@strict_timing
module Aligned {
    in  clk : clock
    in  x   : u32
    out sum : u32
    reg a    : Delayed<u32, 1> = 0
    reg b    : Delayed<u32, 2> = 0
    reg a_d2 : Delayed<u32, 2> = 0
    on clk { a <= x  b <= a  a_d2 <= a }
    sum = a_d2 + b
}

test "delay adds no register" {
    let dut = Claimed { };
    // x is 10, 20, 30, 40 before edges 1-4. After edge 4: a = 40 (x4),
    // b = 30 (x3). delay<1>(a) is a itself (no register): 40 + 30 = 70.
    dut.x = 10;
    step(1);
    dut.x = 20;
    step(1);
    dut.x = 30;
    step(1);
    dut.x = 40;
    step(1);
    assert_eq(dut.sum, 70);
}

test "a stage register aligns the two values" {
    let dut = Aligned { };
    // After edge 4: a_d2 = a after edge 3 = 30 (x3), b = 30 (x3): both
    // are 2 cycles old, 30 + 30 = 60.
    dut.x = 10;
    step(1);
    dut.x = 20;
    step(1);
    dut.x = 30;
    step(1);
    dut.x = 40;
    step(1);
    assert_eq(dut.sum, 60);
}

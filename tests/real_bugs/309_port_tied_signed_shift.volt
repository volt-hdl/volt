//~ PASS
// #309: a signed `>>` whose amount is a port, tied to a constant by the
// instance. Volt emits `a >>> k` (no W2013: the amount is a run-time value
// in Volt). Verilator inlines G and propagates 64 into the shift; 5.050-5.052
// then give 0 for a negative `a` (the oversized constant shift of #302),
// while synthesis, Yosys and `volt verify` give -1. ADR-0036 §1 and
// ADR-0120 (by the width or more, every bit is the sign bit; IEEE
// 1800-2017 §11.4.10): the value is -1 for a negative operand and 0
// otherwise. Fixed by moving the referee Verilator to 5.054 (ADR-0121),
// the pull request that closes #309.
module G {
    in  a : i63
    in  k : u32
    out y : i63
    y = a >> k
}

module P {
    in  a : i63
    out y : i63
    let g = G { a: a, k: 64 }
    y = g.y
}

test "a negative value fills with the sign bit" {
    let dut = P { };
    // -1 in i63 is 0x7FFF_FFFF_FFFF_FFFF: shifted by 64 >= 63 every bit is
    // the sign bit, 1, so the result is again 63 ones (-1). On 5.052: 0.
    dut.a = 0x7FFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.y, 0x7FFF_FFFF_FFFF_FFFF);
}

test "a non-negative value gives 0" {
    let dut = P { };
    // 2^62 - 1, the largest i63: sign bit 0, the result is 0.
    dut.a = 0x3FFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.y, 0);
}

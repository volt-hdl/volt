//~ W2013
// #302: `a >> 64` on an i63 was written `a >>> 64` in the SystemVerilog.
// Verilator 5.050-5.052 (the pinned release is 5.052) gives 0 for a
// negative `a` when a constant amount is at or past the C++ word width
// (64 here), while synthesis, Yosys and `volt verify` give -1: `volt test`
// disagreed with the hardware. ADR-0036 §1 (a signed `>>` is arithmetic)
// and ADR-0120 (by the width or more, every bit is the sign bit; IEEE
// 1800-2017 §11.4.10): the value is -1 for a negative operand and 0
// otherwise. W2013 warns that the amount is at least the width. Fixed in
// the pull request of ADR-0120: the amount is written `w - 1` (62).
module M {
    in  a : i63
    out y : i63
    y = a >> 64
}

test "a negative value fills with the sign bit" {
    let dut = M { };
    // -1 in i63 is 0x7FFF_FFFF_FFFF_FFFF: every bit is the sign bit, 1, so
    // the result is again 63 ones (-1). Before the fix: 0.
    dut.a = 0x7FFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.y, 0x7FFF_FFFF_FFFF_FFFF);
}

test "a non-negative value gives 0" {
    let dut = M { };
    // 2^62 - 1, the largest i63: sign bit 0, the result is 0.
    dut.a = 0x3FFF_FFFF_FFFF_FFFF;
    step(1);
    assert_eq(dut.y, 0);
}

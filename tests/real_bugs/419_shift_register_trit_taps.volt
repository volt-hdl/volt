//~ PASS
// #419: the `taps` of `ShiftRegister<Trit, LEN>` was typed `bits<LEN>` in
// the type checker while the SV declared it 2 × LEN bits wide. Assigning
// it to a `bits<4>` port compiled clean and the SV truncated it
// (`assign window = 4'(sr_taps);`), keeping two of the four stages; the
// right `bits<8>` port was E2001. ADR-0029 types `taps` as
// `bits<LEN * width(T)>` and a `Trit` is two bits (ADR-0003, ADR-0062:
// +1 = 01, 0 = 00, -1 = 11), so `taps` is `bits<8>` here. The type checker
// counted a `Trit` with no width as one bit (`unwrap_or(1)`). Fixed by the
// pull request that closes #419: the width of a type comes from one
// function.

module T {
    in  clk    : clock
    in  up     : bool
    in  down   : bool
    in  strobe : bool
    out window : bits<8>
    out oldest : i2

    let t_in : Trit = if up { 1 } else if down { -1 } else { 0 }

    let sr = ShiftRegister<Trit, 4> {
        clk: clk,
        data_in: t_in,
        shift_en: strobe,
    }

    window = sr.taps
    oldest = sr.data_out as i2
}

// Expected values by hand from docs/stdlib.md (ShiftRegister): each
// shift_en cycle moves data_in into the youngest stage, data_out is the
// oldest stage, taps holds all four stages with the oldest in the top
// slice; the stages reset to the default encoding of T (ADR-0087), the
// Trit 0 = 00. An `out` port reads back its bit pattern: i2 -1 is 3.
test "the taps of ShiftRegister<Trit, 4> are eight bits" {
    let dut = T { };
    // After reset every stage is 0: {00, 00, 00, 00}.
    assert_eq(dut.window, 0);
    dut.strobe = true;
    dut.up = true;
    dut.down = false;
    step(1);
    dut.up = false;
    dut.down = true;
    step(1);
    dut.up = false;
    dut.down = false;
    step(1);
    dut.up = true;
    dut.down = false;
    step(1);
    // In order +1, -1, 0, +1: oldest +1, then -1, 0, youngest +1:
    // {01, 11, 00, 01} = 0b0111_0001 = 113. The bug's bits<4> port kept
    // the low four bits, 0b0001 = 1.
    assert_eq(dut.window, 113);
    assert_eq(dut.oldest, 1);
    dut.strobe = false;
    step(1);
    // No shift: the stages hold.
    assert_eq(dut.window, 113);
    dut.strobe = true;
    dut.up = false;
    dut.down = true;
    step(1);
    // -1 enters, the oldest +1 leaves: {11, 00, 01, 11} = 0b1100_0111 = 199;
    // the oldest stage is now -1, the i2 pattern 3.
    assert_eq(dut.window, 199);
    assert_eq(dut.oldest, 3);
}

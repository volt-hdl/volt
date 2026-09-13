// Simulation tests for the FIR filter family (ADR-0033). The sibling
// fir_filter.volt provides `Fir8` (FirFilter<8, 16>, 1-cycle latency),
// `Fir4` (FirFilter<4, 16>) and `FirFilterPipe` (3-cycle latency).
//
// Timing model: the harness holds rst for 2 cycles, then every
// `dut.port = v` is applied before the next `step`, and asserts read
// the values after the last eval. A sample driven before edge k is in
// taps_r[0] after edge k and `result` is the combinational MAC of the
// taps, so the impulse response is visible one step per coefficient.
//
// TestExpr has no negative literals (ADR-0033); the negative half of
// the response is covered by the `sample == -32768` cover target.

test "fir8 impulse response" {
    let dut = Fir8 { };
    dut.sample = 1;
    dut.valid_in = true;
    step(1);
    dut.sample = 0;
    assert_true(dut.valid_out);
    assert_eq(dut.result, 1);
    step(1);
    assert_eq(dut.result, 2);
    step(1);
    assert_eq(dut.result, 3);
    step(1);
    assert_eq(dut.result, 4);
    step(1);
    assert_eq(dut.result, 4);
    step(1);
    assert_eq(dut.result, 3);
    step(1);
    assert_eq(dut.result, 2);
    step(1);
    assert_eq(dut.result, 1);
    step(1);
    assert_eq(dut.result, 0);
    assert_true(dut.valid_out);
}

test "fir8 step response" {
    let dut = Fir8 { };
    dut.sample = 1000;
    dut.valid_in = true;
    step(1);
    assert_eq(dut.result, 1000);
    step(1);
    assert_eq(dut.result, 3000);
    step(1);
    assert_eq(dut.result, 6000);
    step(1);
    assert_eq(dut.result, 10000);
    step(1);
    assert_eq(dut.result, 14000);
    step(1);
    assert_eq(dut.result, 17000);
    step(1);
    assert_eq(dut.result, 19000);
    step(1);
    assert_eq(dut.result, 20000);
    step(4);
    assert_eq(dut.result, 20000);
    assert_true(dut.valid_out);
}

test "fir8 full scale hits the kernel bound" {
    let dut = Fir8 { };
    dut.sample = 32767;
    dut.valid_in = true;
    step(8);
    // 20 * 32767 — the 32-bit accumulator holds the full product sum.
    assert_eq(dut.result, 655340);
    assert_true(dut.valid_out);
}

test "fir8 zero input stays zero" {
    let dut = Fir8 { };
    dut.sample = 0;
    dut.valid_in = true;
    step(8);
    assert_eq(dut.result, 0);
    assert_true(dut.valid_out);
    dut.valid_in = false;
    step(1);
    assert_eq(dut.result, 0);
    assert_false(dut.valid_out);
}

test "fir8 valid gates the tap line" {
    let dut = Fir8 { };
    dut.sample = 500;
    dut.valid_in = true;
    step(1);
    assert_eq(dut.result, 500);
    // valid low: the tap line holds, result holds, valid_out drops.
    dut.sample = 9000;
    dut.valid_in = false;
    step(3);
    assert_eq(dut.result, 500);
    assert_false(dut.valid_out);
    // valid high again: 9000 enters, 500 shifts to tap 1.
    dut.valid_in = true;
    step(1);
    assert_eq(dut.result, 10000);
    assert_true(dut.valid_out);
}

test "fir4 impulse response uses the first four taps" {
    let dut = Fir4 { };
    dut.sample = 1;
    dut.valid_in = true;
    step(1);
    dut.sample = 0;
    assert_eq(dut.result, 1);
    step(1);
    assert_eq(dut.result, 2);
    step(1);
    assert_eq(dut.result, 3);
    step(1);
    assert_eq(dut.result, 4);
    step(1);
    // The impulse has left the 4-tap line.
    assert_eq(dut.result, 0);
    assert_true(dut.valid_out);
}

test "fir4 step response saturates at gain 10" {
    let dut = Fir4 { };
    dut.sample = 1000;
    dut.valid_in = true;
    step(1);
    assert_eq(dut.result, 1000);
    step(1);
    assert_eq(dut.result, 3000);
    step(1);
    assert_eq(dut.result, 6000);
    step(1);
    assert_eq(dut.result, 10000);
    step(4);
    assert_eq(dut.result, 10000);
}

test "fir4 valid gates the tap line" {
    let dut = Fir4 { };
    dut.sample = 500;
    dut.valid_in = true;
    step(1);
    assert_eq(dut.result, 500);
    dut.sample = 9000;
    dut.valid_in = false;
    step(2);
    assert_eq(dut.result, 500);
    assert_false(dut.valid_out);
    dut.valid_in = true;
    step(1);
    assert_eq(dut.result, 10000);
    assert_true(dut.valid_out);
}

test "pipeline latency" {
    let dut = FirFilterPipe { };
    dut.sample = 1;
    dut.valid_in = true;
    step(1);
    dut.sample = 0;
    // edge 1: sample in the tap line, nothing valid yet
    assert_false(dut.valid_out);
    assert_eq(dut.result, 0);
    step(1);
    // edge 2: products registered (Multiply → Add1), still not valid
    assert_false(dut.valid_out);
    assert_eq(dut.result, 0);
    step(1);
    // edge 3: sums registered (Add1 → Add2): first coefficient visible
    assert_true(dut.valid_out);
    assert_eq(dut.result, 1);
    step(1);
    assert_eq(dut.result, 2);
    step(1);
    assert_eq(dut.result, 3);
    step(1);
    assert_eq(dut.result, 4);
    step(1);
    assert_eq(dut.result, 4);
    step(1);
    assert_eq(dut.result, 3);
    step(1);
    assert_eq(dut.result, 2);
    step(1);
    assert_eq(dut.result, 1);
    step(1);
    assert_eq(dut.result, 0);
}

test "pipeline valid follows input by 3 cycles" {
    let dut = FirFilterPipe { };
    dut.sample = 100;
    dut.valid_in = true;
    step(3);
    assert_true(dut.valid_out);
    // taps after edge 1 were [100,0,...]: the value that reaches
    // result after 3 edges is C0 * 100. The 3-tap sum (600) shows
    // two edges later.
    assert_eq(dut.result, 100);
    step(2);
    assert_eq(dut.result, 600);
    dut.valid_in = false;
    step(1);
    assert_true(dut.valid_out);
    step(1);
    assert_true(dut.valid_out);
    step(1);
    assert_false(dut.valid_out);
}

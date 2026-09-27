// Simulation tests for Counter. A file named X_test.volt sees the
// modules of X.volt, so counter.volt is loaded automatically.
//
// `let dut = Counter { }` instantiates the design with its reset
// applied; `dut.<input> = value` drives an input, step(n) advances the
// clock n cycles, and assert_eq / assert_true / assert_false check
// outputs.

test "counts while enabled" {
    let dut = Counter { };
    dut.enable = true;
    step(3);
    assert_eq(dut.count, 3);
}

test "holds while disabled" {
    let dut = Counter { };
    dut.enable = true;
    step(2);
    dut.enable = false;
    step(5);
    assert_eq(dut.count, 2);
}

test "wraps to zero after MAX" {
    let dut = Counter { };
    dut.enable = true;
    step(9);
    assert_eq(dut.count, 9);
    assert_true(dut.wrap);
    step(1);
    assert_eq(dut.count, 0);
    assert_false(dut.wrap);
}

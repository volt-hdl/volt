// Simulation tests for EventCounter. The test bench drives every clock
// of the design together, one edge per step().

test "counts nothing without pulses" {
    let dut = EventCounter { };
    step(10);
    assert_eq(dut.count, 0);
}

test "counts pulses that are far enough apart" {
    let dut = EventCounter { };
    for i in 0..3 {
        dut.pulse = true;
        step(1);
        dut.pulse = false;
        step(5);
    }
    assert_eq(dut.count, 3);
}

test "a crossing takes a few cycles" {
    let dut = EventCounter { };
    dut.pulse = true;
    step(1);
    dut.pulse = false;
    assert_eq(dut.count, 0);
    step(5);
    assert_eq(dut.count, 1);
}
